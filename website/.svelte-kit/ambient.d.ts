
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
	export const CSF_XCAFDefaults: string;
	export const LESS: string;
	export const MOTD_SHOWN: string;
	export const USER: string;
	export const CSF_StandardLiteDefaults: string;
	export const CSF_ShadersDirectory: string;
	export const OLDPWD: string;
	export const PATH: string;
	export const EDITOR: string;
	export const KITTY_PID: string;
	export const XDG_BACKEND: string;
	export const COLORTERM: string;
	export const XDG_VTNR: string;
	export const HYPRCURSOR_SIZE: string;
	export const XDG_SESSION_CLASS: string;
	export const QT_QPA_PLATFORMTHEME: string;
	export const DRAWDEFAULT: string;
	export const DEBUGINFOD_URLS: string;
	export const GTK_THEME: string;
	export const PAGER: string;
	export const WAYLAND_DISPLAY: string;
	export const QT_STYLE_OVERRIDE: string;
	export const STARSHIP_SHELL: string;
	export const CSF_STEPDefaults: string;
	export const CSF_DrawPluginDefaults: string;
	export const CSF_MIGRATION_TYPES: string;
	export const TERMINFO: string;
	export const HYPRLAND_INSTANCE_SIGNATURE: string;
	export const SHELL: string;
	export const ZSH: string;
	export const CASROOT: string;
	export const MASON: string;
	export const GDK_BACKEND: string;
	export const PWD: string;
	export const CSF_EXCEPTION_PROMPT: string;
	export const MAIL: string;
	export const CSF_MDTVTexturesDirectory: string;
	export const CSF_OCCTResourcePath: string;
	export const LOGNAME: string;
	export const KITTY_WINDOW_ID: string;
	export const LS_COLORS: string;
	export const DIRHISTORY_SIZE: string;
	export const VISUAL: string;
	export const CSF_TObjMessage: string;
	export const CSF_PluginDefaults: string;
	export const CSF_XSMessage: string;
	export const HL_INITIAL_WORKSPACE_TOKEN: string;
	export const GTK_ICON_THEME: string;
	export const VIMRUNTIME: string;
	export const LANG: string;
	export const CSF_IGESDefaults: string;
	export const XCURSOR_SIZE: string;
	export const _: string;
	export const _JAVA_AWT_WM_NONREPARENTING: string;
	export const HYPRLAND_CMD: string;
	export const XDG_RUNTIME_DIR: string;
	export const LSCOLORS: string;
	export const TERM: string;
	export const MMGT_CLEAR: string;
	export const XDG_SESSION_TYPE: string;
	export const CSF_LANGUAGE: string;
	export const KITTY_INSTALLATION_DIR: string;
	export const GOPATH: string;
	export const KITTY_PUBLIC_KEY: string;
	export const DBUS_SESSION_BUS_ADDRESS: string;
	export const CSF_XmlOcafResource: string;
	export const MOZ_ENABLE_WAYLAND: string;
	export const NVIM: string;
	export const CSF_TObjDefaults: string;
	export const XCURSOR_THEME: string;
	export const SHLVL: string;
	export const DRAWHOME: string;
	export const HOME: string;
	export const CSF_SHMessage: string;
	export const DISPLAY: string;
	export const XDG_CURRENT_DESKTOP: string;
	export const MYVIMRC: string;
	export const HYPRCURSOR_THEME: string;
	export const STARSHIP_SESSION_KEY: string;
	export const XDG_SESSION_ID: string;
	export const NVIM_LOG_FILE: string;
	export const CSF_StandardDefaults: string;
	export const XDG_SEAT: string;
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
		CSF_XCAFDefaults: string;
		LESS: string;
		MOTD_SHOWN: string;
		USER: string;
		CSF_StandardLiteDefaults: string;
		CSF_ShadersDirectory: string;
		OLDPWD: string;
		PATH: string;
		EDITOR: string;
		KITTY_PID: string;
		XDG_BACKEND: string;
		COLORTERM: string;
		XDG_VTNR: string;
		HYPRCURSOR_SIZE: string;
		XDG_SESSION_CLASS: string;
		QT_QPA_PLATFORMTHEME: string;
		DRAWDEFAULT: string;
		DEBUGINFOD_URLS: string;
		GTK_THEME: string;
		PAGER: string;
		WAYLAND_DISPLAY: string;
		QT_STYLE_OVERRIDE: string;
		STARSHIP_SHELL: string;
		CSF_STEPDefaults: string;
		CSF_DrawPluginDefaults: string;
		CSF_MIGRATION_TYPES: string;
		TERMINFO: string;
		HYPRLAND_INSTANCE_SIGNATURE: string;
		SHELL: string;
		ZSH: string;
		CASROOT: string;
		MASON: string;
		GDK_BACKEND: string;
		PWD: string;
		CSF_EXCEPTION_PROMPT: string;
		MAIL: string;
		CSF_MDTVTexturesDirectory: string;
		CSF_OCCTResourcePath: string;
		LOGNAME: string;
		KITTY_WINDOW_ID: string;
		LS_COLORS: string;
		DIRHISTORY_SIZE: string;
		VISUAL: string;
		CSF_TObjMessage: string;
		CSF_PluginDefaults: string;
		CSF_XSMessage: string;
		HL_INITIAL_WORKSPACE_TOKEN: string;
		GTK_ICON_THEME: string;
		VIMRUNTIME: string;
		LANG: string;
		CSF_IGESDefaults: string;
		XCURSOR_SIZE: string;
		_: string;
		_JAVA_AWT_WM_NONREPARENTING: string;
		HYPRLAND_CMD: string;
		XDG_RUNTIME_DIR: string;
		LSCOLORS: string;
		TERM: string;
		MMGT_CLEAR: string;
		XDG_SESSION_TYPE: string;
		CSF_LANGUAGE: string;
		KITTY_INSTALLATION_DIR: string;
		GOPATH: string;
		KITTY_PUBLIC_KEY: string;
		DBUS_SESSION_BUS_ADDRESS: string;
		CSF_XmlOcafResource: string;
		MOZ_ENABLE_WAYLAND: string;
		NVIM: string;
		CSF_TObjDefaults: string;
		XCURSOR_THEME: string;
		SHLVL: string;
		DRAWHOME: string;
		HOME: string;
		CSF_SHMessage: string;
		DISPLAY: string;
		XDG_CURRENT_DESKTOP: string;
		MYVIMRC: string;
		HYPRCURSOR_THEME: string;
		STARSHIP_SESSION_KEY: string;
		XDG_SESSION_ID: string;
		NVIM_LOG_FILE: string;
		CSF_StandardDefaults: string;
		XDG_SEAT: string;
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
