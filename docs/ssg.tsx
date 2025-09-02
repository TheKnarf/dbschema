#!/usr/bin/env -S VITE_NODE=true vite-node --script
//
//  This file is distributed under the MIT License
//
import { build } from 'vite';
import ssgPlugin from './ssg-for-vite.tsx';

try {
  console.log("Running Vite build with SSG setup (Static site generation)\n");

  const routes = (await import('./src/routes')).default;

  // Hack so that we can have different options
  // for vite-node and for the build under
  process.env.VITE_NODE = false;

  const ssgPluginConfig = {
    routes,
    mainScript: '/ssg-main.tsx',
  }

  const viteConfig = (await import('./vite.config.ts')).default;

  let originalViteConfig =
    typeof viteConfig == 'function' ? viteConfig() : viteConfig;

  await build({
    ...originalViteConfig,
    plugins: [
      ssgPlugin(ssgPluginConfig),
      ...(originalViteConfig.plugins),
    ],
  });
} catch(e) {
  console.error(e);
  process.exit(1);
}
