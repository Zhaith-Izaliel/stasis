
// this file is generated — do not edit it


/// <reference types="@sveltejs/kit" />

/**
 * Environment variables [loaded by Vite](https://vitejs.dev/guide/env-and-mode.html#env-files) from `.env` files and `process.env`. Like [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private), this module cannot be imported into client-side code. This module only includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured).
 * 
 * _Unlike_ [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private), the values exported from this module are statically injected into your bundle at build time, enabling optimisations like dead code elimination.
 * 
 * ```ts
 * import { API_KEY } from '$env/static/private';
 * ```
 * 
 * Note that all environment variables referenced in your code should be declared (for example in an `.env` file), even if they don't have a value until the app is deployed:
 * 
 * ```
 * MY_FEATURE_FLAG=""
 * ```
 * 
 * You can override `.env` values from the command line like so:
 * 
 * ```sh
 * MY_FEATURE_FLAG="enabled" npm run dev
 * ```
 */
declare module '$env/static/private' {
	export const HYPRCURSOR_SIZE: string;
	export const XDG_SESSION_CLASS: string;
	export const SSH_AGENT_PID: string;
	export const TERMINFO: string;
	export const QT_QPA_PLATFORMTHEME: string;
	export const DISPLAY: string;
	export const GTK_THEME: string;
	export const WAYLAND_DISPLAY: string;
	export const STARSHIP_SHELL: string;
	export const HYPRLAND_INSTANCE_SIGNATURE: string;
	export const XDG_VTNR: string;
	export const ZSH: string;
	export const PATH: string;
	export const MASON: string;
	export const GDK_BACKEND: string;
	export const PWD: string;
	export const COLORTERM: string;
	export const MAIL: string;
	export const XDG_RUNTIME_DIR: string;
	export const KITTY_WINDOW_ID: string;
	export const LS_COLORS: string;
	export const SHELL: string;
	export const DIRHISTORY_SIZE: string;
	export const XDG_BACKEND: string;
	export const HL_INITIAL_WORKSPACE_TOKEN: string;
	export const VIMRUNTIME: string;
	export const GTK_ICON_THEME: string;
	export const VISUAL: string;
	export const LOGNAME: string;
	export const XCURSOR_SIZE: string;
	export const DBUS_SESSION_BUS_ADDRESS: string;
	export const KITTY_PID: string;
	export const _JAVA_AWT_WM_NONREPARENTING: string;
	export const HYPRLAND_CMD: string;
	export const SHLVL: string;
	export const _: string;
	export const SSH_AUTH_SOCK: string;
	export const LSCOLORS: string;
	export const GOPATH: string;
	export const TERM: string;
	export const XDG_SESSION_TYPE: string;
	export const KITTY_INSTALLATION_DIR: string;
	export const LESS: string;
	export const KITTY_PUBLIC_KEY: string;
	export const USER: string;
	export const MOZ_ENABLE_WAYLAND: string;
	export const LANG: string;
	export const MOTD_SHOWN: string;
	export const HOME: string;
	export const XDG_CURRENT_DESKTOP: string;
	export const MYVIMRC: string;
	export const OLDPWD: string;
	export const STARSHIP_SESSION_KEY: string;
	export const EDITOR: string;
	export const NVIM_LOG_FILE: string;
	export const NVIM: string;
	export const XDG_SEAT: string;
	export const XDG_SESSION_ID: string;
	export const DEBUGINFOD_URLS: string;
	export const QT_STYLE_OVERRIDE: string;
	export const PAGER: string;
	export const NODE_ENV: string;
}

/**
 * Similar to [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private), except that it only includes environment variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`), and can therefore safely be exposed to client-side code.
 * 
 * Values are replaced statically at build time.
 * 
 * ```ts
 * import { PUBLIC_BASE_URL } from '$env/static/public';
 * ```
 */
declare module '$env/static/public' {
	
}

/**
 * This module provides access to runtime environment variables, as defined by the platform you're running on. For example if you're using [`adapter-node`](https://github.com/sveltejs/kit/tree/main/packages/adapter-node) (or running [`vite preview`](https://svelte.dev/docs/kit/cli)), this is equivalent to `process.env`. This module only includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured).
 * 
 * This module cannot be imported into client-side code.
 * 
 * ```ts
 * import { env } from '$env/dynamic/private';
 * console.log(env.DEPLOYMENT_SPECIFIC_VARIABLE);
 * ```
 * 
 * > [!NOTE] In `dev`, `$env/dynamic` always includes environment variables from `.env`. In `prod`, this behavior will depend on your adapter.
 */
declare module '$env/dynamic/private' {
	export const env: {
		HYPRCURSOR_SIZE: string;
		XDG_SESSION_CLASS: string;
		SSH_AGENT_PID: string;
		TERMINFO: string;
		QT_QPA_PLATFORMTHEME: string;
		DISPLAY: string;
		GTK_THEME: string;
		WAYLAND_DISPLAY: string;
		STARSHIP_SHELL: string;
		HYPRLAND_INSTANCE_SIGNATURE: string;
		XDG_VTNR: string;
		ZSH: string;
		PATH: string;
		MASON: string;
		GDK_BACKEND: string;
		PWD: string;
		COLORTERM: string;
		MAIL: string;
		XDG_RUNTIME_DIR: string;
		KITTY_WINDOW_ID: string;
		LS_COLORS: string;
		SHELL: string;
		DIRHISTORY_SIZE: string;
		XDG_BACKEND: string;
		HL_INITIAL_WORKSPACE_TOKEN: string;
		VIMRUNTIME: string;
		GTK_ICON_THEME: string;
		VISUAL: string;
		LOGNAME: string;
		XCURSOR_SIZE: string;
		DBUS_SESSION_BUS_ADDRESS: string;
		KITTY_PID: string;
		_JAVA_AWT_WM_NONREPARENTING: string;
		HYPRLAND_CMD: string;
		SHLVL: string;
		_: string;
		SSH_AUTH_SOCK: string;
		LSCOLORS: string;
		GOPATH: string;
		TERM: string;
		XDG_SESSION_TYPE: string;
		KITTY_INSTALLATION_DIR: string;
		LESS: string;
		KITTY_PUBLIC_KEY: string;
		USER: string;
		MOZ_ENABLE_WAYLAND: string;
		LANG: string;
		MOTD_SHOWN: string;
		HOME: string;
		XDG_CURRENT_DESKTOP: string;
		MYVIMRC: string;
		OLDPWD: string;
		STARSHIP_SESSION_KEY: string;
		EDITOR: string;
		NVIM_LOG_FILE: string;
		NVIM: string;
		XDG_SEAT: string;
		XDG_SESSION_ID: string;
		DEBUGINFOD_URLS: string;
		QT_STYLE_OVERRIDE: string;
		PAGER: string;
		NODE_ENV: string;
		[key: `PUBLIC_${string}`]: undefined;
		[key: `${string}`]: string | undefined;
	}
}

/**
 * Similar to [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private), but only includes variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`), and can therefore safely be exposed to client-side code.
 * 
 * Note that public dynamic environment variables must all be sent from the server to the client, causing larger network requests — when possible, use `$env/static/public` instead.
 * 
 * ```ts
 * import { env } from '$env/dynamic/public';
 * console.log(env.PUBLIC_DEPLOYMENT_SPECIFIC_VARIABLE);
 * ```
 */
declare module '$env/dynamic/public' {
	export const env: {
		[key: `PUBLIC_${string}`]: string | undefined;
	}
}
