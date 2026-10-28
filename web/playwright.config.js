import {defineConfig} from '@playwright/test';
export default defineConfig({
  testDir: './tests', timeout: 60000, workers: 1,
  webServer: process.env.CUEMOL_WEB_TEST_URL ? undefined : {command: 'cargo run --locked -p cuemol-cli -- web serve --dir web --port 8097 --no-open', cwd: '..', url: 'http://127.0.0.1:8097', timeout: 120000, reuseExistingServer: !process.env.CI},
  use: {baseURL: process.env.CUEMOL_WEB_TEST_URL || 'http://127.0.0.1:8097', viewport: {width:1280,height:900}, screenshot: 'only-on-failure',
    launchOptions: {args:['--enable-unsafe-webgpu',...(process.platform==='linux' ? [
      '--enable-unsafe-swiftshader','--use-gl=angle','--use-angle=swiftshader',
      '--enable-features=Vulkan','--use-vulkan=swiftshader','--disable-vulkan-surface',
    ] : [])]},
  },
});
