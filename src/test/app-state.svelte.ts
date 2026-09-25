// Stand-in for SvelteKit's `$app/state` in component tests.
export const page = $state({ url: new URL("http://localhost/") });

export function setUrl(href: string) {
  page.url = new URL(href, "http://localhost");
}
