/**
 * Content boundary for the docs app. Articles live as Markdown files under
 * `content/{en,es}/` and are listed in `content/index.json` (sections in
 * render order); the glob inlines them at build time, so the app needs no
 * backend in dev or production.
 */
import index from "../content/index.json";

export type Lang = "en" | "es";

export type Localized = { en: string; es: string };

export type Article = {
  slug: string;
  title: Localized;
  summary: Localized;
};

export type Section = {
  id: string;
  title: Localized;
  articles: Article[];
};

export const sections: Section[] = (index as { sections: Section[] }).sections;

export const articles: Article[] = sections.flatMap((s) => s.articles);

export function sectionOf(slug: string): Section | undefined {
  return sections.find((s) => s.articles.some((a) => a.slug === slug));
}

const bodies = import.meta.glob("../content/{en,es}/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

export function articleBody(lang: Lang, slug: string): string {
  return bodies[`../content/${lang}/${slug}.md`] ?? "";
}
