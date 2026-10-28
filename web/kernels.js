import createUsalign from './pkg/kernels/usalign.js';

export async function initKernels() {
  const module = await createUsalign();
  globalThis.cuemolKernels = {
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
