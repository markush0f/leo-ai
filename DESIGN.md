---
name: Leo
description: Grok-style chat — rail, empty canvas, composer — in light and dark themes.
colors:
  bg: "#0c0c0d"
  sidebar: "#141416"
  surface: "#1c1c1f"
  text: "#f2f2f4"
  muted: "#9a9aa3"
  primary: "#f4f4f5"
  primary-fg: "#0c0c0d"
  accent: "#c4b5fd"
  danger: "#f87171"
  listen: "#34d399"
typography:
  display:
    fontFamily: "Source Sans 3 Variable, Source Sans 3, sans-serif"
    fontSize: "2.4rem"
    fontWeight: 650
    lineHeight: 1.15
    letterSpacing: "-0.04em"
  body:
    fontFamily: "Source Sans 3 Variable, Source Sans 3, sans-serif"
    fontSize: "16px"
    fontWeight: 450
    lineHeight: 1.5
  label:
    fontFamily: "Source Sans 3 Variable, Source Sans 3, sans-serif"
    fontSize: "0.9rem"
    fontWeight: 600
    lineHeight: 1.2
rounded:
  control: "10px"
  composer: "22px"
  bubble: "18px"
  send: "999px"
spacing:
  rail: "260px"
  measure: "48rem"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.primary-fg}"
    rounded: "{rounded.control}"
    padding: "0.5rem 0.85rem"
  button-danger:
    backgroundColor: "transparent"
    textColor: "{colors.danger}"
    rounded: "{rounded.control}"
    padding: "0.5rem 0.85rem"
---

# Design System: Leo

## Overview

**Creative North Star: "Grok's shell, Leo's voice"**

The window follows Grok's chat flow: a left rail, a calm empty canvas with "Hola", a floating composer that docks to the bottom after the first turn. User turns sit in a right-aligned bubble; Leo answers as plain text on the left.

Light and dark are first-class. Dark is charcoal like GrokNight; light is paper like GrokDay. Buttons are a small vocabulary you can tell apart at a glance.

**Key Characteristics:**
- Rail + canvas + composer card
- Inverse filled primary vs bordered secondary vs red danger
- Theme toggle persisted in `localStorage`
- No cloud photograph, no rainbow hairlines

## Colors

Neutral surfaces, one accent, semantic action colors.

### Primary
- **Inverse fill** (#f4f4f5 on dark / #111113 on light): Nuevo chat, Enviar, Activar, Añadir

### Secondary
- **Accent** (#c4b5fd / #6d28d9): focus ring and the active model name

### Neutral
- **Bg** (#0c0c0d / #f4f4f5)
- **Sidebar** (#141416 / #ececee)
- **Surface** (#1c1c1f / #ffffff)
- **Text / muted** (#f2f2f4 / #9a9aa3)

### Named Rules
**The Three-Button Rule.** Primary is filled inverse. Secondary is bordered. Danger is red outline. Never restyle all three as the same pill.

## Typography

**Display Font:** Source Sans 3 Variable (650)
**Body Font:** Source Sans 3 Variable (450)
**Label Font:** Source Sans 3 Variable (600)

**Character:** Neutral humanist at readable weights. No extra-light display.

### Hierarchy
- **Hero** (650, 2.4rem): Hola
- **Brand** (650, 1.25rem): Leo
- **Body** (450, 16px, max 48rem): messages
- **Label** (600, 0.9rem): buttons and fields

## Layout

260px rail. Main column. Composer max 48rem centered. On the welcome screen the composer sits under the hero; after the first message it docks to the bottom. Below 860px the rail becomes a drawer.

## Elevation & Depth

Composer and catalog use a soft shadow (`--shadow`). Surfaces are flat otherwise.

## Shapes

Controls 10px. Composer 22px. User bubble 18px with a square inner corner. Send is a 36px circle.

## Components

### Buttons
- **Primary:** filled inverse (Nuevo chat, Activar, Añadir, Enviar)
- **Secondary:** elevated + border (kind)
- **Danger:** red outline, fills red on hover (Borrar)
- **Ghost:** no border (tema, menú, cerrar)

### Inputs / Fields
- Elevated fill, 1px `--border`, 10px radius. Focus: 2px accent outline.

### Navigation
- Rail: Nuevo chat, Catálogo, theme. Esc closes catalog and the mobile rail.

### Composer
- Card with textarea, model `<select>`, circular send.

### Catalog
- Right settings panel + dim scrim. Same button vocabulary.

## Do's and Don'ts

### Do:
- **Do** keep Grok's empty-canvas → docked-composer sequence.
- **Do** ship light and dark from the same tokens.
- **Do** make send, danger, and secondary look like different objects.

### Don't:
- **Don't** put a photograph or iridescent gradient on the chrome.
- **Don't** render every action as the same outlined pill.
- **Don't** hide the theme toggle.
