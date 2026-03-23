# KSBH Docs Site

This directory contains the KSBH documentation site built with Dodeca.

## Current Stack

- Dodeca for content rendering
- Gingembre templates
- Tailwind v4 + DaisyUI for styling
- local font assets in `static/fonts/`

## Working Files

Important paths:

- `.config/dodeca.styx`
- `content/`
- `data/docs_sidebar.yaml`
- `templates/base.html`
- `templates/index.html`
- `templates/section.html`
- `templates/page.html`
- `templates/macros/`
- `templates/partials/`
- `css/base.css`
- `static/css/style.css`

## CSS Workflow

- Docs entry file: `css/base.css`
- Shared CSS source of truth: `../crates/ksbh-ui/static/css/shared.css`
- Shared CSS generated file: `../crates/ksbh-ui/static/css/style.css`
- Generated file: `static/css/style.css`
- One-shot docs-local build: `deno task build:css`
- Start watcher: `deno task dev:css`
- Build both shared UI CSS and docs CSS once: `mise run build-css`

`deno task dev:css` now watches both `css/base.css` and `../crates/ksbh-ui/static/css/shared.css`, and rebuilds both generated CSS outputs.

Do not edit `static/css/style.css` or `../crates/ksbh-ui/static/css/style.css` as source; they are generated output.

## Dodeca Workflow

Common commands:

```bash
ddc serve
ddc serve --no-tui
ddc build
```

Current config in `.config/dodeca.styx` is intentionally small:

```styx
content content
output public

syntax_highlight {
    dark_theme catppuccin-mocha
}
```

## Content Layout

- `content/_index.md` is the site homepage
- docs pages live under `content/docs/...`
- `_index.md` files define sections
- regular `.md` files define pages

## Template Notes

This repo’s templates use `get_section(path=...)` heavily.

The docs left sidebar order is driven by `data/docs_sidebar.yaml`.

- top-level item order controls the section order in the left rail
- nested `pages:` order controls the document order inside each section
- keep paths in that file aligned with Dodeca section/page paths such as `docs/modules/proof-of-work/_index.md`

Do not assume subsection arrays already contain rich section objects. Resolve paths explicitly before reading fields.

Pattern used in this repo:

```html
{% set docs_root = get_section(path="docs/_index.md") %}
{% for doc_section_path in docs_root.subsections %}
{% set doc_section = get_section(path=doc_section_path) %}
<a href="{{ doc_section.permalink }}">{{ doc_section.title }}</a>
{% endfor %}
```

## Validation

For docs-only changes:

- use `ddc serve`
- use the CSS watcher if you changed `base.css`
- do not run Cargo unless you also changed Rust code
