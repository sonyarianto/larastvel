import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Larastvel',
  description: 'A Rust web framework inspired by Laravel, built on Axum, Tokio, and SeaORM',
  ignoreDeadLinks: true,
  base: process.env.VERCEL ? '/' : '/larastvel/',
  themeConfig: {
    logo: '/favicon.svg',
    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'Reference', link: '/reference/cli' },
      { text: 'GitHub', link: 'https://github.com/sonyarianto/larastvel' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'Quick Start', link: '/guide/getting-started' },
            { text: 'Configuration', link: '/guide/configuration' },
            { text: 'Architecture', link: '/guide/architecture' },
          ],
        },
        {
          text: 'Core Concepts',
          items: [
            { text: 'Routing', link: '/guide/routing' },
            { text: 'Database & ORM', link: '/guide/database' },
            { text: 'Authentication', link: '/guide/auth' },
            { text: 'Session & CSRF', link: '/guide/session' },
            { text: 'Caching', link: '/guide/caching' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'CLI Reference',
          link: '/reference/cli',
        },
        {
          text: 'Parity Tracking',
          link: '/reference/parity',
        },
      ],
    },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/sonyarianto/larastvel' },
    ],
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © Sony AK',
    },
  },
})
