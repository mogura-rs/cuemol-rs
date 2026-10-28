let model;
let queue = Promise.resolve();
let current;
async function denoise(job, width, height, color, albedo, normal) {
  if (!navigator.gpu) throw new Error('OIDN requires WebGPU. Select Bilateral or A-trous in this browser.');
  if (!model) {
    model = import('./pkg/oidn/oidn.js').then(({initUNetFromURL}) =>
      initUNetFromURL(new URL('./pkg/oidn/rt_hdr_calb_cnrm.tza', import.meta.url).href,
        undefined, {hdr: true, aux: true, maxTileSize: 256}));
    model.catch(() => {model = undefined;});
  }
  const unet = await model;
  if (job.cancelled) throw new Error('Render cancelled');
  return new Promise((resolve, reject) => {
    job.abort = unet.tileExecute({color: {data: color, width, height}, albedo: {data: albedo, width, height},
      normal: {data: normal, width, height}, done: image => resolve(image.data), error: reject});
  });
}
globalThis.cuemolOidn = {
  run(width, height, color, albedo, normal) {
    const job = {cancelled: false};
    const cancelled = new Promise((_, reject) => {job.cancel = () => {
      job.cancelled = true; job.abort?.(); reject(new Error('Render cancelled'));
    };});
    current = job;
    const result = Promise.race([queue.then(() => denoise(job, width, height, color, albedo, normal)), cancelled]);
    queue = result.catch(() => {}).finally(() => {if (current === job) current = undefined;});
    return result;
  },
  cancel() {current?.cancel();}
};
