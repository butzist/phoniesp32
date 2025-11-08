# Agent Guidelines for phoniesp32/web

## Build/Lint/Test Commands

- **Build for production**: `just build` or `dx build --release`
- **Clean build artifacts**: `just clean`
- **Run full build pipeline**: `just all`
- **Run single test**: No test framework configured - add tests to `src/` files
  with `#[test]` and run `cargo test --lib`
- **Never run**: `dx serve` (not allowed)

## Code Style Guidelines

### Dioxus Framework Usage

- Use Dioxus 0.7+ patterns: signals instead of use_state, RSX syntax,
  #[component] macro
- Import with `use dioxus::prelude::*;` and `use dioxus_bulma as b;`
- Components must be PascalCase, functions snake_case
- Props must be owned values (String/Vec<T>), implement PartialEq + Clone
- Use ReadOnlySignal<T> for reactive props
- Prefer loops over iterators in RSX: `for item in items { ... }` not
  `{(0..5).map(...)}`

### State Management

- Use `use_signal(|| initial_value)` for local component state
- Use `use_memo(move || expensive_calc())` for derived state
- Use `use_context_provider`/`use_context` for shared state
- Use `use_resource(move || async { ... })` for async operations

### UI Components

- Use dioxus-bulma components: `b::Button`, `b::Card`, `b::Input`, etc.
- Wrap app in
  `b::BulmaProvider { theme: b::BulmaTheme::Auto, load_bulma_css: true, ... }`

### Error Handling

- Use `anyhow::Result<T>` for fallible operations
- Use `?` operator and `.context("description")` for error context
- Handle async errors in use_resource with match statements

### Routing

- Define routes with
  `#[derive(Routable)] enum Route { #[route("/path")] Component {} }`
- Use `Router::<Route> {}` and `Outlet::<Route> {}` for navigation
- Enable router feature in dioxus-bulma for navigation components

### Assets

- Use `asset!("/path")` macro for static assets
- Place assets in `assets/` directory
- Use `document::Stylesheet` for external CSS

### Naming Conventions

- Components: PascalCase (MyComponent)
- Functions: snake_case (my_function)
- Variables: snake_case (my_variable)
- Types: PascalCase (MyStruct)
- Enums: PascalCase (MyEnum)

### Imports

- Group dioxus imports: `use dioxus::prelude::*;`
- Use dioxus_bulma as b: `use dioxus_bulma as b;`
- Import specific icons: `use dioxus_free_icons::icons::fa_regular_icons::*;`
- Keep imports organized and minimal

### Formatting

- Use `rustfmt` for consistent formatting
- Use `dx fmt` for formatting the rsx section
- Follow standard Rust indentation and spacing
- Use `#[rustfmt::skip]` for complex route enums

## Dioxus Bulma Overview

Dioxus-bulma provides Bulma CSS components for Dioxus apps. Key components:

- Layout: `b::Container`, `b::Columns`/`b::Column`, `b::Section`
- Elements: `b::Button`, `b::Title`/`b::Subtitle`, `b::Icon`, `b::Image`
- Forms: `b::Input`, `b::Textarea`, `b::Select`, `b::Field`/`b::Control`
- Components: `b::Card`, `b::Modal`, `b::Dropdown`, `b::Notification`

### Router-Enabled Components

When router feature is enabled, these components support `to` prop for
navigation:

- `b::Button` - Navigate on click instead of form submission
- `b::BreadcrumbItem` - Router-aware breadcrumb navigation
- `b::DropdownItem` - Navigate from dropdown menus
- `b::MenuItem` - Navigate from vertical menus
- `b::PanelBlock` - Navigate from panel items
- `b::PaginationPrevious`/`b::PaginationNext`/`b::PaginationLink` - Navigate
  between pages

### Navigation Example

````rust
b::Button {
    color: b::BulmaColor::Primary,
    to: Route::Playback {},
    "Go Playback"
}
```

### Colors / Palette

- Any buttons and text are allowed to use only Bulma color palette classes (e.g., has-text-primary), avoid custom colors.

### Grid System Hints

Use `b::Columns` and `b::Column` for responsive layouts:
- `size: b::ColumnSize::Half` for 50% width columns
- `size: b::ColumnSize::OneThird` for 33% width columns
- Responsive breakpoints: `IsFullMobile`, `IsHalfTablet`, `IsOneQuarterDesktop`
- Add `multiline: true` to `b::Columns` for wrapping columns

### Forms Hints

Structure forms with `b::Field` and `b::Control`:
- Wrap inputs in `b::Field { b::Control { b::Input { ... } } }`
- Add `b::Label` above fields and `b::Help` below for guidance
- Use `grouped: true` on `b::Field` for horizontal button groups
- Validation: set `color: b::BulmaColor::Success` or `b::BulmaColor::Danger`

### Router Outlet Hints

Use `Outlet::<Route> {}` in layout components to render child routes:
- Define layouts with `#[layout(ComponentName)]` on parent routes
- Place `Outlet::<Route> {}` where child content should appear
- Use `#[layout(NavBar)]` for shared navigation across multiple routes

See https://github.com/rexlunae/dioxus-bulma/blob/main/README.md for detailed examples and documentation.
````
