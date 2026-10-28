import createUsalign from './pkg/kernels/usalign.js';
import createLcms from './pkg/kernels/lcms.js';
import createSurface from './pkg/kernels/surface.js';

export async function initKernels() {
  const [module, cms, ses] = await Promise.all([createUsalign(), createLcms(), createSurface()]);
  globalThis.cuemolKernels = {
    surface(xyzr, density, probe, backend) {
      const data = ses._malloc(xyzr.byteLength), error = ses._malloc(512);
      let handle = 0;
      try {
        if (!data || !error) throw new Error('Surface allocation failed');
        ses.HEAPF64.set(xyzr, data / 8);ses.HEAPU8[error] = 0;
        handle = ses._cuemol_surface_build(data, xyzr.length / 4, density, probe, backend, error, 512);
        if (!handle) throw new Error(ses.UTF8ToString(error) || 'Surface generation failed');
        const vertices = ses._cuemol_surface_vertex_count(handle), triangles = ses._cuemol_surface_triangle_count(handle);
        const v = ses._cuemol_surface_vertices(handle) / 8, t = ses._cuemol_surface_triangles(handle) / 4;
        return [ses.HEAPF64.slice(v,v+vertices*6), ses.HEAPU32.slice(t,t+triangles*3), ses._cuemol_surface_backend(handle)];
      } finally {
        if (handle) ses._cuemol_surface_destroy(handle);
        ses._free(data);ses._free(error);
      }
    },
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
