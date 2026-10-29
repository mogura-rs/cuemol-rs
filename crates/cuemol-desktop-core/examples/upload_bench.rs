use cuemol_desktop_core::viewport::ReusableGpuBuffer;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "direct".into());
    let count: usize = args.next().as_deref().unwrap_or("1000000").parse()?;
    let iterations: usize = args.next().as_deref().unwrap_or("10").parse()?;
    if !matches!(mode.as_str(), "direct" | "collected") || count == 0 || iterations == 0 {
        return Err("expected direct|collected, positive item count and iteration count".into());
    }
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
    let mut buffer = ReusableGpuBuffer::new(
        &device,
        "benchmark",
        wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_SRC,
    );
    let value = |index: usize| std::array::from_fn::<f32, 20, _>(|axis| (index + axis) as f32);
    let started = Instant::now();
    for _ in 0..iterations {
        if mode == "direct" {
            buffer.upload_iter(&device, &queue, count, (0..count).map(value));
        } else {
            let values: Vec<_> = (0..count).map(value).collect();
            buffer.upload(&device, &queue, &values);
            std::hint::black_box(&values);
        }
        queue.submit([]);
        device.poll(wgpu::PollType::wait_indefinitely())?;
    }
    let elapsed = started.elapsed();
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("benchmark-readback"),
        size: 240,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    let indices = [0, count / 2, count - 1];
    for (target, source) in indices.into_iter().enumerate() {
        encoder.copy_buffer_to_buffer(
            &buffer.buffer,
            source as u64 * 80,
            &readback,
            target as u64 * 80,
            80,
        );
    }
    queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap()
        });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    receiver.recv()??;
    {
        let bytes = readback.slice(..).get_mapped_range();
        for (expected, actual) in indices.into_iter().map(value).zip(bytes.chunks_exact(80)) {
            assert_eq!(bytemuck::bytes_of(&expected), actual);
        }
    }
    readback.unmap();
    println!("mode={mode} items={count} iterations={iterations} bytes_per_item=80 ms_per_upload={:.3} readback=exact adapter={:?}", elapsed.as_secs_f64() * 1000.0 / iterations as f64, adapter.get_info().name);
    Ok(())
}
