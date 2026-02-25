// @ts-check
import { themes as prismThemes } from 'prism-react-renderer';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Klock',
  tagline: 'The Concurrency Control Plane for the Agent Economy',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://docs.klock.ai',
  baseUrl: '/',

  organizationName: 'klock-protocol',
  projectName: 'klock',

  onBrokenLinks: 'throw',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: './sidebars.js',
          editUrl: 'https://github.com/waythor-lab/klock/tree/main/Klock-OpenSource/docs-site/',
        },
        blog: false, // Disable blog for now
        theme: {
          customCss: './src/css/custom.css',
        },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      colorMode: {
        defaultMode: 'dark',
        respectPrefersColorScheme: true,
      },
      navbar: {
        title: 'Klock',
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'docsSidebar',
            position: 'left',
            label: 'Docs',
          },
          {
            href: 'https://www.npmjs.com/package/@klock-protocol/core',
            label: 'NPM',
            position: 'right',
          },
          {
            href: 'https://pypi.org/project/klock/',
            label: 'PyPI',
            position: 'right',
          },
          {
            href: 'https://github.com/waythor-lab/klock',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Documentation',
            items: [
              { label: 'Getting Started', to: '/docs/getting-started' },
              { label: 'Architecture', to: '/docs/architecture' },
              { label: 'API Reference', to: '/docs/api-reference' },
            ],
          },
          {
            title: 'SDKs',
            items: [
              { label: 'JavaScript', to: '/docs/sdk/javascript' },
              { label: 'Python', to: '/docs/sdk/python' },
            ],
          },
          {
            title: 'More',
            items: [
              { label: 'Benchmarks', to: '/docs/benchmarks' },
              { label: 'GitHub', href: 'https://github.com/waythor-lab/klock' },
            ],
          },
        ],
        copyright: `Copyright © ${new Date().getFullYear()} Klock Protocol. MIT License.`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ['rust', 'bash', 'python', 'json', 'yaml', 'toml'],
      },
    }),
};

export default config;
