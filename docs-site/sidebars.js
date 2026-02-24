/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docsSidebar: [
    'getting-started',
    'architecture',
    'api-reference',
    {
      type: 'category',
      label: 'SDKs',
      items: ['sdk/javascript', 'sdk/python'],
    },
    'benchmarks',
    {
      type: 'category',
      label: 'Deployment',
      items: ['deployment/docker'],
    },
  ],
};

export default sidebars;
