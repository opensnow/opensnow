// @ts-check
import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://opensnow.io',
  redirects: {
    '/charts': '/charts/opensnow',
  },
});
