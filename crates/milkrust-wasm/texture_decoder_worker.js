/* Static image decoding worker. Kept as a separate package artifact so strict
 * CSPs can allow a same-origin worker without `blob:` in worker-src. */
self.onmessage = async ({ data }) => {
  let bitmap = null;
  try {
    bitmap = await createImageBitmap(data.blob, {
      imageOrientation: 'flipY',
      premultiplyAlpha: 'none',
    });
    const width = bitmap.width;
    const height = bitmap.height;
    const pixels = width * height;
    if (!width || !height || width > 4096 || height > 4096 || pixels > 8388608 ||
        !((width === data.width && height === data.height) ||
          (width === data.height && height === data.width))) {
      throw new Error('decoded dimensions exceed admission');
    }
    self.postMessage({ bitmap }, [bitmap]);
    bitmap = null;
  } catch (_) {
    self.postMessage({ error: 'Image decode failed or exceeded its admitted dimensions' });
  } finally {
    if (bitmap) bitmap.close();
  }
};
