import {defineConfig} from '@playwright/test';
import {browserLaunchArgs} from './tests/browser-options.js';
const server = process.env.CUEMOL_WEB_TEST_PREBUILT === '1'
  ? 'target/debug/cuemol'
  : 'cargo run --locked -p cuemol-cli --';
export default defineConfig({
  testDir: './tests', timeout: 60000, workers: 1, fullyParallel: true,
  webServer: process.env.CUEMOL_WEB_TEST_URL ? undefined : {command: `${server} web serve --dir web --port 8097 --no-open`, cwd: '..', url: 'http://127.0.0.1:8097', timeout: 120000, reuseExistingServer: !process.env.CI},
  use: {baseURL: process.env.CUEMOL_WEB_TEST_URL || 'http://127.0.0.1:8097', viewport: {width:1280,height:900}, screenshot: 'only-on-failure', trace: 'retain-on-failure',
    launchOptions: {args:browserLaunchArgs},
  },
});
