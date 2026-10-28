import createUsalign from './pkg/kernels/usalign.js';
import createLcms from './pkg/kernels/lcms.js';

export async function initKernels() {
  const [module, cms] = await Promise.all([createUsalign(), createLcms()]);
  globalThis.cuemolKernels = {
    cmsCreate(bytes, intent) {
      const data = cms._malloc(bytes.length);
      if (!data) throw new Error('ICC profile allocation failed');
      try {
        cms.HEAPU8.set(bytes, data);
        const handle = cms._cuemol_cms_create(data, bytes.length, intent);
        if (!handle) throw new Error('LittleCMS rejected the ICC profile');
        return handle;
      } finally { cms._free(data); }
    },
    cmsDestroy(handle) { cms._cuemol_cms_destroy(handle); },
    cmsColor(handle, rgb, gamut) { return cms._cuemol_cms_color(handle, rgb, gamut ? 1 : 0); },
    cmsInfo(handle) {
      const values = [];
      for (let type = 0; type < 4; type++) {
        const size = cms._cuemol_cms_info(handle, type, 0, 0);
        if (!size) continue;
        const output = cms._malloc(size);
        if (!output) throw new Error('ICC description allocation failed');
        try {
          cms._cuemol_cms_info(handle, type, output, size);
          const text = cms.UTF8ToString(output).trim();
          if (text) values.push(text);
        } finally { cms._free(output); }
      }
      return values.join('\n');
    },
    usalign(moving, movingNames, reference, referenceNames, pivot) {
      const pointers = [];
      const allocate = bytes => {
        const pointer = module._malloc(bytes);
        if (!pointer) throw new Error('USalign allocation failed');
        pointers.push(pointer);
        return pointer;
      };
      const copy = array => {
        const pointer = allocate(array.byteLength);
        module.HEAPU8.set(new Uint8Array(array.buffer, array.byteOffset, array.byteLength), pointer);
        return pointer;
      };
      try {
        const mx = copy(moving), mn = copy(movingNames);
        const rx = copy(reference), rn = copy(referenceNames);
        const atom = copy(new TextEncoder().encode(pivot + '\0'));
        const matrix = allocate(96), scores = allocate(32), error = allocate(512);
        module.HEAPU8[error] = 0;
        const status = module._cuemol_usalign(mx, mn, moving.length / 3,
          rx, rn, reference.length / 3, atom, matrix, scores, error, 512);
        if (status) throw new Error(module.UTF8ToString(error) || 'USalign failed');
        const output = new Float64Array(16);
        output.set(module.HEAPF64.subarray(matrix / 8, matrix / 8 + 12));
        output.set(module.HEAPF64.subarray(scores / 8, scores / 8 + 4), 12);
        return output;
      } finally {
        for (const pointer of pointers) module._free(pointer);
      }
    },
  };
}
