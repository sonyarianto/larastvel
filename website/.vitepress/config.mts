import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Larastvel',
  description: 'A Rust web framework inspired by Laravel, built on Axum, Tokio, and SeaORM',
  ignoreDeadLinks: true,
  base: process.env.VERCEL ? '/' : '/larastvel/',
  themeConfig: {
    appearance: 'dark',
    logo: '/logo.png',
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
            { text: 'Directory Structure', link: '/guide/structure' },
            { text: 'Architecture', link: '/guide/architecture' },
            { text: 'Deployment', link: '/guide/deployment' },
          ],
        },
        {
          text: 'Core Concepts',
          items: [
            { text: 'Routing', link: '/guide/routing' },
            { text: 'Middleware', link: '/guide/middleware' },
            { text: 'Controllers', link: '/guide/controllers' },
            { text: 'Views & Templating', link: '/guide/views' },
            { text: 'Validation', link: '/guide/validation' },
            { text: 'Error Handling', link: '/guide/errors' },
            { text: 'Logging', link: '/guide/logging' },
          ],
        },
        {
          text: 'Security',
          items: [
            { text: 'Authentication', link: '/guide/auth' },
            { text: 'Authorization', link: '/guide/authorization' },
            { text: 'Session & CSRF', link: '/guide/session' },
            { text: 'Encryption & Hashing', link: '/guide/encryption' },
            { text: 'Password Reset', link: '/guide/passwords' },
          ],
        },
        {
          text: 'Database',
          items: [
            { text: 'Database & ORM', link: '/guide/database' },
            { text: 'Migrations', link: '/guide/migrations' },
            { text: 'Pagination', link: '/guide/pagination' },
          ],
        },
        {
          text: 'Digging Deeper',
          items: [
            { text: 'Caching', link: '/guide/caching' },
            { text: 'Arr', link: '/guide/arr' },
            { text: 'Collections', link: '/guide/collections' },
            { text: 'Date & Time', link: '/guide/datetime' },
            { text: 'Events', link: '/guide/events' },
            { text: 'Broadcasting', link: '/guide/broadcasting' },
            { text: 'File Storage', link: '/guide/filesystem' },
            { text: 'HTTP Client', link: '/guide/http-client' },
            { text: 'Localization', link: '/guide/localization' },
            { text: 'Mail', link: '/guide/mail' },
            { text: 'Notifications', link: '/guide/notifications' },
            { text: 'Queues', link: '/guide/queues' },
            { text: 'Rate Limiting', link: '/guide/rate-limiting' },
            { text: 'Str', link: '/guide/str' },
            { text: 'Task Scheduling', link: '/guide/scheduling' },
            { text: 'SMS', link: '/guide/sms' },
          ],
        },
        {
          text: 'Testing',
          items: [
            { text: 'Testing', link: '/guide/testing' },
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
