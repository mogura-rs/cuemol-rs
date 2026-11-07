const destinations = new Map();
let sequence = 0;

async function pick(suggestedName) {
  let handle;
  if (typeof showSaveFilePicker === 'function') {
    try {
      handle = await showSaveFilePicker({
        id: 'cuemol-image', suggestedName,
        types: [{description: 'PNG image', accept: {'image/png': ['.png']}}],
      });
    } catch (error) {
      if (error?.name === 'AbortError') return null;
      throw error;
    }
  }
  const id = String(++sequence);
  const name = handle?.name || suggestedName;
  destinations.set(id, {handle, name});
  return {id, name};
}

function release(id) {
  destinations.delete(id);
}

async function write(id, bytes, signal) {
  const destination = destinations.get(id);
  if (!destination) throw new Error('Image export destination has expired');
  let writer;
  try {
    signal.throwIfAborted();
    if (!destination.handle) {
      const url = URL.createObjectURL(new Blob([bytes], {type: 'image/png'}));
      const link = document.createElement('a');
      link.href = url;
      link.download = destination.name;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 60000);
      return;
    }
    writer = await destination.handle.createWritable();
    signal.throwIfAborted();
    await writer.write(bytes);
    signal.throwIfAborted();
    await writer.close();
    writer = undefined;
  } catch (error) {
    if (writer) await writer.abort().catch(() => {});
    throw error;
  } finally {
    release(id);
  }
}

globalThis.cuemolImageExports = {pick, write, release};
