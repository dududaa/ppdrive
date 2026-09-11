// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { fileURLToPath } from 'node:url';

const satteriWasmStub = fileURLToPath(new URL('./satteri-wasi-stub.mjs', import.meta.url));

// https://astro.build/config
export default defineConfig({
	site: 'https://ppdrive.dududaa.org',
	vite: {
		resolve: {
			alias: [{ find: '@bruits/satteri-wasm32-wasi', replacement: satteriWasmStub }],
		},
	},
	integrations: [
		starlight({
			title: 'PPDRIVE',
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/dududaa/ppdrive' },
				{ icon: 'discord', label: 'Discord', href: 'https://discord.gg/6nB4xYnxeC' },
			],
			sidebar: [
				{ label: 'Introduction', slug: 'introduction' },
				{
					label: 'Getting Started',
					items: [
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Quick Start', slug: 'getting-started/quick-start' },
					],
				},
				{
					label: 'Configuration',
					items: [
						{ label: 'Config File', slug: 'configuration/config-file' },
						{ label: 'Secrets & Security', slug: 'configuration/secrets' },
						{ label: 'Database', slug: 'configuration/database' },
					],
				},
				{
					label: 'CLI Reference',
					items: [
						{ label: 'Overview', slug: 'cli/overview' },
						{ label: 'Client Commands', slug: 'cli/client' },
						{ label: 'Bucket Commands', slug: 'cli/bucket' },
						{ label: 'User Commands', slug: 'cli/user' },
						{ label: 'Asset Commands', slug: 'cli/asset' },
						{ label: 'Serve & Configure', slug: 'cli/serve-configure' },
					],
				},
				{
					label: 'API Reference',
					items: [
						{ label: 'Authentication', slug: 'api/authentication' },
						{ label: 'User Authentication', slug: 'api/user-auth' },
						{ label: 'Bucket Management', slug: 'api/buckets' },
						{ label: 'Upload Flow', slug: 'api/upload' },
						{ label: 'Download Flow', slug: 'api/download' },
						{ label: 'File Permissions', slug: 'api/permissions' },
						{ label: 'MIME Validation', slug: 'api/mime-validation' },
					],
				},
				{
					label: 'Architecture',
					items: [
						{ label: 'Overview', slug: 'architecture/overview' },
						{ label: 'Crate Structure', slug: 'architecture/crates' },
						{ label: 'Security Model', slug: 'architecture/security' },
					],
				},
				{
					label: 'Deployment',
					items: [
						{ label: 'Production Setup', slug: 'deployment/production' },
						{ label: 'Docker', slug: 'deployment/docker' },
					],
				},
				{
					label: 'Contributing',
					items: [
						{ label: 'Development Setup', slug: 'contributing/development' },
						{ label: 'Codebase Walkthrough', slug: 'contributing/codebase' },
						{ label: 'SDK Contributor Guide', slug: 'contributing/sdk' },
					],
				},
			],
		}),
	],
});
