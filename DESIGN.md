---
name: Ira
description: Conversación violeta, expresiva y precisa; lectura tranquila y controles compartidos.
colors:
  bg: "#16121f"
  sidebar: "#21192f"
  surface: "#231d30"
  elevated: "#30263e"
  border: "#443650"
  text: "#f4effa"
  muted: "#b7abc8"
  accent: "#c1a5ff"
  primary: "#b99af5"
  primary-fg: "#211239"
  danger: "#ff9b9b"
  listen: "#8bd9bc"
  user-bubble: "#6545a5"
  light-bg: "#f7f4fc"
  light-sidebar: "#eee7f7"
  light-surface: "#ffffff"
  light-elevated: "#f1ebf8"
  light-border: "#d8cce6"
  light-text: "#291b3d"
  light-muted: "#71617e"
  light-accent: "#6940a9"
  light-primary: "#7048b3"
  light-primary-fg: "#ffffff"
  light-danger: "#b72e43"
  light-listen: "#237257"
  light-user-bubble: "#7048b3"
typography:
  display:
    fontFamily: "Source Sans 3 Variable, Source Sans 3, system-ui, sans-serif"
    fontSize: "clamp(1.9rem, 3.2vw, 2.9rem)"
    fontWeight: 620
    lineHeight: 1.12
    letterSpacing: "-0.035em"
  title:
    fontSize: "1.6rem"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "-0.03em"
  body:
    fontFamily: "Source Sans 3 Variable, Source Sans 3, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 450
    lineHeight: 1.5
  reply:
    fontSize: "16px"
    fontWeight: 450
    lineHeight: 1.7
  button:
    fontSize: "0.9rem"
    fontWeight: 600
    lineHeight: 1
  label:
    fontSize: "0.84rem"
    fontWeight: 600
  field:
    fontSize: "0.94rem"
  code:
    fontFamily: "ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "0.82rem"
    lineHeight: 1.45
rounded:
  chip: "8px"
  control: "10px"
  field: "11px"
  action: "12px"
  composer: "16px"
  pill: "999px"
spacing:
  compact: "0.5rem"
  small: "0.75rem"
  base: "1rem"
  gutter: "1.5rem"
  section: "2rem"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.primary-fg}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    padding: "0.5rem 0.85rem"
  button-secondary:
    backgroundColor: "{colors.elevated}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "0.5rem 0.85rem"
  button-danger:
    backgroundColor: "transparent"
    textColor: "{colors.danger}"
    rounded: "{rounded.control}"
    padding: "0.5rem 0.85rem"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "0.5rem 0.85rem"
  input-field:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.text}"
    typography: "{typography.field}"
    rounded: "{rounded.field}"
    padding: "0.7rem 0.85rem"
  navigation:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "0.55rem 0.7rem"
  mode-chip:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.chip}"
    padding: "0.4rem 0.55rem"
  composer:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.composer}"
    padding: "1rem 1rem 0.8rem"
---

# Design System: Ira

## Overview

**Creative North Star: "Conversación violeta, expresiva y precisa"**

Berenjena oscura y lavanda clara prolongan la identidad del logo existente. Controles esculpidos, superficies abiertas y respuestas sin tarjeta mantienen el contenido por delante de la interfaz. Dirección fijada por el usuario y construida desde código; sin assets generados. Se conserva `/ira-logo.png` sin cambios.

El movimiento señala navegación, envío y actividad real. Durante la lectura, los mensajes permanecen en reposo y el desplazamiento respeta a quien vuelve atrás.

**Key Characteristics:**
- Dos temas semánticos con acento violeta.
- Sidebar ajustable y compactable; campos compartidos con etiquetas persistentes.
- Burbuja violeta para Tú; avatar y texto abierto para Ira.
- Movimiento breve, streaming agrupado y reducción de movimiento respetada.

Fuente normativa: cascada completa de `desktop/src/styles.css`, `App.tsx`, `Catalog.tsx`, `Databases.tsx`, `icons.tsx` y `components/{Field,Composer,Message,Activity,useSidebar,useSheetFocus}`. Contrato: `.impeccable/surfaces/desktop-index-html.md`; comportamiento implementado prevalece sobre medidas preliminares.

## Colors

### Primary
`primary` rellena acciones; `primary-fg` aporta contraste. `accent` marca foco, modelo activo y actividad. `user-bubble` diferencia la voz del usuario. Coral queda en detalles de marca, no en acciones ni estados.

### Neutral
`bg`, `sidebar`, `surface` y `elevated` separan planos; `border` delimita controles; `text` y `muted` jerarquizan lectura. Tokens sin prefijo corresponden a oscuro; `light-*` refleja la misma variable CSS bajo `[data-theme="light"]`. Los componentes del frontmatter describen oscuro; en claro sustituyen cada rol por su homólogo. Los snippets usan variables CSS vivas.

`danger` identifica errores y eliminación; `listen`, escucha o resultado correcto. Estado siempre acompañado de texto. Las rampas del sidecar son ayudas de exploración, no colores adicionales autorizados: neutros reutilizan planos existentes; acentos y estados usan rampas OKLCH ilustrativas.

**The Semantic Color Rule.** Mantener roles equivalentes entre temas; reservar coral a la marca y comunicar estados también con texto.

## Typography

Source Sans 3 Variable sostiene bienvenida, conversación y controles; monoespaciada solo para código. `display` corresponde a bienvenida, `title` a paneles, `body` a interfaz, `reply` a Ira y `label` a campos. Tamaños menores no se convierten en titulares.

En móvil: bienvenida usa `clamp(1.9rem, 5vw, 2.6rem)` hasta 860px; títulos de panel, 1.4rem hasta 600px. Markdown conserva jerarquía contenida: h1/h2 de 1.15em, otros títulos de 1em; bloques de código y tablas desplazan horizontalmente.

## Layout

- Sidebar: 272px iniciales, ajuste 224–360px, compacto 76px desde 861px. `useSidebar` persiste `ira.rail.width` y `ira.rail.collapsed` en `localStorage`; tolera almacenamiento indisponible. Flechas ajustan 16px, Home/End llegan a límites; doble clic restaura ancho inicial.
- Hasta 860px: drawer de `min(310px, calc(100vw - 48px))`, menú visible, sin tirador. Cabecera de 76px pasa a 64px.
- Bienvenida: dock de hasta 48rem. Conversación: dock hasta 52rem, mensajes hasta 50rem; gutters de 1.5rem. Son máximos externos, incluido padding, no ancho neto del texto.
- Paneles Catálogo/Bases de datos: hasta 850px, cabecera sticky, navegación de 11rem y detalle flexible. Hasta 600px: una columna, navegación horizontal, formularios apilados y gutter de 1rem. Acciones de base de datos permanecen sticky.
- Hasta 1080px se oculta texto de chips, conservando controles accesibles. Móvil respeta `safe-area-inset-bottom`; ventanas bajas permiten scroll de bienvenida.

## Elevation & Depth

Profundidad por capas tonales y bordes finos. Compositor sin sombra; panel lateral con `-18px 0 70px rgba(9, 4, 18, 0.22)`, scrim negro al 45%. Tarjeta de voz usa `--shadow` dependiente del tema. Valores completos en sidecar; no heredar sombras antiguas anuladas por la cascada.

## Shapes

Radios del frontmatter separan botones, campos, acciones cuadradas y compositor. Enviar/micrófono miden 40px, con radio `action`; no son círculos. Burbuja del usuario tiene esquina de salida: `16px 16px 4px 16px`. Pills quedan en usos concretos como «Ir al final», no como forma universal.

## Components

### Botones, iconos y campos
Primario violeta; secundario elevado con borde; peligro transparente con borde semántico; ghost discreto. Hover primario aclara, secundario cambia plano, peligro rellena, ghost revela superficie. Pulsación desplaza 1px; deshabilitado pierde opacidad. Foco global: contorno de 2px y offset de 2px.

`icons.tsx` centraliza Hugeicons (`size={20}`, `strokeWidth={1.7}`, decorativos con `aria-hidden`); CSS ajusta tamaños por contexto. Controles solo-icono conservan nombre accesible.

`Field.tsx` comparte `Input`, `TextArea` y `Select`: label asociado por ID, ayuda/error mediante `aria-describedby`, `aria-invalid`, icono opcional y mostrar/ocultar contraseña. Control de mínimo 46px; textarea de mínimo 110px. Foco cambia borde/plano, dibuja línea inferior y añade contorno de teclado con offset de 3px. Error usa `danger`; disabled reduce opacidad del contenedor.

### Compositor, mensajes y actividad
`Composer` crece hasta 176px; Enter envía, Shift+Enter crea línea, composición IME no envía. Modelo, pensamiento, herramientas, voz y envío comparten barra; chips activos usan tinte violeta. Dock pasa de bienvenida a anclaje inferior mediante layout de posición.

`Message` está memoizado: Tú a derecha en burbuja; Ira con logo, Markdown abierto e interlineado `reply`; error con `role="alert"`. `Activity` representa espera/recepción con cuatro barras decorativas; texto informa estado. Streaming se agrupa por `requestAnimationFrame`, con vaciado final y anuncio al completar. Autoscroll solo si se sigue respuesta (menos de 96px del final); «Ir al final» recupera seguimiento.

### Paneles y conexiones
`Catalog` reutiliza campos para proveedores, modelos y configuración. `Databases` agrupa Destino, Credenciales y Opciones avanzadas. `parseDatabaseUrl` procesa localmente `postgres://`/`postgresql://`: decodifica credenciales, admite IPv6, puerto válido y solo `sslmode` reconocido; rechaza fragmentos y opciones desconocidas con errores que no repiten la URL.

URL y contraseña permanecen en estado React transitorio, sin persistencia frontend ni logging del parser. Importar rellena formulario y vacía URL; guardar envía campos estructurados al backend y limpia contraseña del borrador. Contraseña vacía en edición conserva la guardada. Esto no significa que las credenciales nunca se envíen: se transmiten al guardar, no al interpretar URL.

«Guardar y comprobar» separa `saving` y `testing`: primero guarda, luego prueba por ID. Fallar prueba conserva conexión editable; resultado previo se limpia al editar. Restricciones nativas abren Opciones avanzadas si un campo oculto resulta inválido. Confirmación explícita para eliminar.

`useSheetFocus` lleva foco al primer control visible, contiene Tab/Shift+Tab y devuelve foco al disparador o menú visible disponible. Paneles y drawer móvil abierto son diálogos modales; fondo `inert`. Escape cierra panel/drawer/voz. Drawer cerrado en móvil queda oculto para interacción.

### Movimiento
`MotionConfig reducedMotion="user"`: spring general 380/36 (stiffness/damping), paneles 360/38, mensajes 420/34 y selección de chat 420/38. Animar entrada/selección, no lectura ni cada delta. CSS de controles usa 150–280ms; actividad espera 1200ms y recepción 650ms.

Excepción acotada de layout: transición `width 300ms cubic-bezier(0.16, 1, 0.3, 1)` para contraer/expandir sidebar; desactivada durante arrastre por puntero y con movimiento reducido. En móvil se usa transform de 200ms. No extender esta excepción a otros cambios de tamaño.

`prefers-reduced-motion: reduce` anula animaciones/transiciones CSS y scroll suave; Motion respeta preferencia, mensajes omiten entrada y dock omite layout. Indicador conserva forma estática diferenciada.

### Evidencia de cierre
Handoff recibido: revisión completa, tres correcciones materiales resueltas; disposición final **ship al alcance de fix-list**. Siete pruebas Playwright con mocks y `npm run build` aprobados; warning existente del chunk principal (~571KB). No acredita conexión PostgreSQL real ni voz real.

Capturas validadas en cierre: `.impeccable/review/{desktop,desktop-chat,desktop-database,mobile,mobile-database,mobile-drawer}.png`. Para repetir comprobación funcional: desde `desktop`, `npm test` y `npm run build` (fuera del pase documental). Verificar esquema y referencias de este documento junto con `.impeccable/design.json` al regenerarlos.

## Do's and Don'ts

### Do:
- **Do** conservar logo existente, Hugeicons y etiquetas accesibles.
- **Do** reutilizar Field y roles de color en ambos temas.
- **Do** distinguir guardar de comprobar y mantener lectura quieta durante streaming.

### Don't:
- **Don't** recuperar la estética Grok monocroma ni las antiguas franjas iridiscentes.
- **Don't** persistir URL o credenciales crudas en almacenamiento frontend.
- **Don't** convertir movimiento continuo o sombras en decoración de cada mensaje.
