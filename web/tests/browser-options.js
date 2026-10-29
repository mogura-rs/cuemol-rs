export const browserLaunchArgs = ['--enable-unsafe-webgpu',...(process.platform==='linux' ? [
  '--enable-unsafe-swiftshader','--use-gl=angle','--use-angle=swiftshader',
  '--enable-features=Vulkan','--use-vulkan=swiftshader','--disable-vulkan-surface',
] : [])];
