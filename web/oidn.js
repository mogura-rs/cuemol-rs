let model;
let queue = Promise.resolve();
async function denoise(width, height, color, albedo, normal) {
  if (!navigator.gpu) throw new Error('OIDN requires WebGPU. Select Bilateral or A-trous in this browser.');
  if (!model) {
    model = import('./pkg/oidn/oidn.js').then(({initUNetFromURL}) =>
      initUNetFromURL(new URL('./pkg/oidn/rt_hdr_calb_cnrm.tza', import.meta.url).href,
        undefined, {hdr: true, aux: true, maxTileSize: 256}));
    model.catch(() => {model = undefined;});
  }
  const unet = await model;
  return new Promise((resolve, reject) => {
    unet.tileExecute({color: {data: color, width, height}, albedo: {data: albedo, width, height},
      normal: {data: normal, width, height}, done: image => resolve(image.data), error: reject});
  });
}
globalThis.cuemolOidn = {
  run(width, height, color, albedo, normal) {
    const result = queue.then(() => denoise(width, height, color, albedo, normal));
    queue = result.catch(() => {});
    return result;
  }
};
