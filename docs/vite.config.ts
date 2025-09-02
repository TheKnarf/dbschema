import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react-swc';
import mdx from '@mdx-js/rollup'
import svgr from 'vite-plugin-svgr'; // https://github.com/pd4d10/vite-plugin-svgr
import rehypePrettyCode from 'rehype-pretty-code';
import remarkFrontmatter from 'remark-frontmatter';
import remarkMdxFrontmatter from 'remark-mdx-frontmatter';
import remarkGfm from 'remark-gfm';
import { remarkAlert } from 'remark-github-blockquote-alert';
import { splitVendorChunkPlugin } from 'vite';
import { ViteImageOptimizer } from 'vite-plugin-image-optimizer';

/** @type {import('rehype-pretty-code').Options} */
const rehypePrettyCodeOptions = {
  theme: 'solarized-light',
};

export default defineConfig(() => {
  const config = {
    root: 'src',
    build: {
      outDir: '../dist',
      emptyOutDir: true,
    },
    publicDir: '../public',

    plugins: [
      splitVendorChunkPlugin(),
      ViteImageOptimizer({}),
      {enforce: 'pre', ...mdx({
        remarkPlugins: [
          remarkFrontmatter,
          remarkMdxFrontmatter,
          remarkGfm,
          remarkAlert,
        ],
        rehypePlugins: [
          [rehypePrettyCode, rehypePrettyCodeOptions],
        ],
      })},
      svgr(),
      react()
    ],
  };


  return config;
});
