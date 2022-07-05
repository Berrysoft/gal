import { convertFileSrc, invoke } from "@tauri-apps/api/tauri"
import { Locale } from 'vue-i18n'

export interface Info {
    title: string,
    author: string,
}

export interface Action {
    line: string,
    character: string | null,
    switches: Array<Switch>,
    bg: string | undefined,
    bgm: string | undefined,
}

export interface Switch {
    text: string,
    enabled: boolean,
}

export function choose_locale(locales: Locale[]): Promise<Locale | null> {
    return invoke("choose_locale", { locales: locales })
}

export function locale_native_name(loc: Locale): Promise<string> {
    return invoke("locale_native_name", { loc: loc })
}

export function info(): Promise<Info> {
    return invoke("info")
}

export function start_new(locale: Locale): Promise<void> {
    return invoke("start_new", { locale: locale })
}

export function next_run(): Promise<boolean> {
    return invoke("next_run")
}

export async function current_run(): Promise<Action | null> {
    let res = await invoke<Action | null>("current_run")
    if (res != null) {
        if (res.bg != undefined) {
            res.bg = convertFileSrc(res.bg)
        }
        if (res.bgm != undefined) {
            res.bgm = convertFileSrc(res.bgm)
        }
    }
    return res
}

export function switch_(i: number): Promise<void> {
    return invoke("switch", { i: i })
}