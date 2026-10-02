---
name: Ira
description: Conversación violeta, expresiva y precisa; lectura tranquila, controles compartidos y espacios de trabajo por tarea.
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
  workspace-title:
    fontSize: "1.75rem"
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
  task-navigation:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "0.6rem 1rem"
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

Berenjena oscura y lavanda clara prolongan la identidad del logo existente. Controles esculpidos, superficies abiertas y respuestas sin tarjeta mantienen el contenido por delante de la interfaz. Dirección fijada por el usuario y construida desde código; sin assets generados. Se conservan los assets de marca; la interfaz usa `/ira-cabeza-recortada.png`.

El movimiento señala navegación, envío y actividad real. Durante la lectura, los mensajes permanecen en reposo y el desplazamiento respeta a quien vuelve atrás.

**Key Characteristics:**
- Dos temas semánticos con acento violeta.
- Sidebar ajustable y compactable; campos compartidos con etiquetas persistentes.
- Espacios de trabajo no modales; listas de recursos con estado, acción y detalles progresivos.
- Burbuja violeta para Tú; avatar y texto abierto para Ira.
- Movimiento breve, streaming agrupado y reducción de movimiento respetada.

Fuente normativa: `desktop/src/styles.css` y utilidades Tailwind de `desktop/src/ui.ts`, con comportamiento en `App.tsx`, `Catalog.tsx`, `Services.tsx`, `Databases.tsx`, `McpConnections.tsx` y `components/{Workspace,ModelPicker,Status,Activity,Composer,Message}`. Contrato: `.impeccable/surfaces/desktop-index-html.md`; comportamiento implementado prevalece sobre medidas preliminares y la cascada anterior. El usuario confirmó cualquier usuario como audiencia, conservar violeta/logo/temas y referencias ChatGPT/Claude; aprobó el plan y pidió «Implementa». La estrategia de esta superficie permanece en su contrato.

## Colors

### Primary
`primary` rellena acciones; `primary-fg` aporta contraste. `accent` marca foco, modelo activo y actividad. `user-bubble` diferencia la voz del usuario. Coral queda en detalles de marca, no en acciones ni estados.

### Neutral
`bg`, `sidebar`, `surface` y `elevated` separan planos; `border` delimita controles; `text` y `muted` jerarquizan lectura. Tokens sin prefijo corresponden a oscuro; `light-*` refleja la misma variable CSS bajo `[data-theme="light"]`. Los componentes del frontmatter describen oscuro; en claro sustituyen cada rol por su homólogo. Los snippets usan variables CSS vivas.

`danger` identifica errores y eliminación; `listen`, escucha o resultado correcto. Estado siempre acompañado de texto. Las rampas del sidecar son ayudas de exploración, no colores adicionales autorizados: neutros reutilizan planos existentes; acentos y estados usan rampas OKLCH ilustrativas.

**The Semantic Color Rule.** Mantener roles equivalentes entre temas; reservar coral a la marca y comunicar estados también con texto.

## Typography

Source Sans 3 Variable sostiene bienvenida, conversación y controles; monoespaciada solo para código. `display` corresponde a bienvenida, `title` al panel de documentación, `workspace-title` a Ajustes/Conexiones, `body` a interfaz, `reply` a Ira y `label` a campos. El nuevo rol de cabecera hereda la misma familia; no sustituye el título del panel. Tamaños menores no se convierten en titulares.

En móvil: bienvenida usa `clamp(1.9rem, 5vw, 2.6rem)` hasta 860px; títulos de panel y workspace, 1.4rem hasta 600px. Cabeceras de recursos usan 1.2rem/600; secciones de Ajustes, 1.4rem/600. Markdown conserva jerarquía contenida: h1/h2 de 1.15em, otros títulos de 1em; bloques de código y tablas desplazan horizontalmente.

## Layout

- Sidebar: 272px iniciales, ajuste 224–360px, compacto 76px desde 861px. `useSidebar` persiste `ira.rail.width` y `ira.rail.collapsed` en `localStorage`; tolera almacenamiento indisponible. Flechas ajustan 16px, Home/End llegan a límites; doble clic restaura ancho inicial.
- Hasta 860px: drawer de `min(310px, calc(100vw - 48px))`, menú visible, sin tirador. Cabecera de 76px pasa a 64px.
- Bienvenida: dock de hasta 48rem. Conversación: dock y mensajes hasta 50rem; gutters de 1.5rem, 1rem hasta 600px. Son máximos externos, incluido padding, no ancho neto del texto. Se conserva el anclaje inferior al conversar y el seguimiento condicional del scroll.
- Ajustes/Conexiones ocupan el área junto al sidebar; chat oculto conserva conversación y compositor. Workspace sin scroll exterior, cabecera y navegación fijas y cuerpo centrado hasta 1060px. Solo el contenido de cada apartado tiene scroll: detalle de Ajustes o recursos de Conexiones, con overscroll contenido y espacio estable para scrollbar. Padding de cabecera 1.8rem 2.5rem, cuerpo 2rem 2.5rem y 3rem inferiores dentro del contenido desplazable. Ajustes usa navegación de 11rem y detalle flexible con gap de 2.5rem. Conexiones usa filtros que pueden envolver y listas abiertas; Sistema de Ira aparece solo en su filtro. En el sidebar solo desplaza la lista de conversaciones; su encabezado y los accesos de sección permanecen fijos.
- Hasta 860px: workspace con gutter de 1.25rem. Hasta 600px: gutter de 1rem, Ajustes en una columna con navegación horizontal envolvente, formularios apilados y acciones de recurso en fila propia. Cabecera conserva botón de cierre accesible cuando se oculta «Volver al chat»; padding inferior respeta safe area. Acciones del editor son de flujo normal, no sticky.
- Selector de modelo: diálogo en portal y capa nativa superior, ancho de hasta 400px limitado a viewport menos 32px, margen mínimo de 16px y separación de 8px del disparador. Elige espacio superior/inferior, mide alto real y reajusta con ResizeObserver y resize; alto máximo hasta 480px según espacio disponible, lista con scroll hasta 280px.
- Hasta 1080px se oculta texto de chips, conservando controles accesibles. Móvil respeta `safe-area-inset-bottom`; ventanas bajas permiten scroll de bienvenida.

## Elevation & Depth

Profundidad por capas tonales y bordes finos. Compositor y workspaces sin sombra; panel lateral de documentación con `-18px 0 70px rgba(9, 4, 18, 0.22)`, scrim negro al 45%. Selector de modelo y tarjeta de voz usan `--shadow-float` dependiente del tema; backdrop del selector negro al 25%. Valores completos en sidecar; no trasladar sombra o scrim modal a Ajustes/Conexiones.

## Shapes

Radios del frontmatter separan botones, campos, acciones cuadradas y compositor. Enviar/micrófono miden 40px, 44px hasta 600px, con radio `action`; no son círculos. Burbuja del usuario tiene esquina de salida: `16px 16px 4px 16px`. Editores de recurso y selector usan radio de 14px; listas abiertas se separan con borde inferior. Pills quedan en usos concretos como «Ir al último mensaje», no como forma universal.

## Components

### Botones, iconos y campos
Primario violeta; secundario elevado con borde; peligro transparente con borde semántico; ghost discreto. Hover primario aclara, secundario cambia plano, peligro rellena, ghost revela superficie. Pulsación desplaza 1px; deshabilitado pierde opacidad. Foco global: contorno de 2px y offset de 2px.

`icons.tsx` centraliza Hugeicons (`size={20}`, `strokeWidth={1.7}`, decorativos con `aria-hidden`); CSS ajusta tamaños por contexto. Controles solo-icono conservan nombre accesible.

`Field.tsx` comparte `Input`, `TextArea` y `Select`: label asociado por ID, ayuda/error mediante `aria-describedby`, `aria-invalid`, icono opcional y mostrar/ocultar contraseña. Control de mínimo 46px; textarea de mínimo 110px. Foco cambia borde/plano, dibuja línea inferior y añade contorno de teclado con offset de 3px. Error usa `danger`; disabled reduce opacidad del contenedor.

### Compositor, mensajes y actividad
`Composer` crece hasta 176px; Enter envía, Shift+Enter crea línea, composición IME no envía. Selector de modelo, potencia cuando está disponible, herramientas, escritura, voz y envío comparten barra; chips activos usan tinte violeta. Dock pasa de bienvenida a anclaje inferior mediante layout de posición.

`Message` está memoizado: Tú a derecha en burbuja; Ira con logo, Markdown abierto e interlineado `reply`; error con `role="alert"`. `Activity` usa SVG orbital decorativo de 20px; texto informa «Preparando tu respuesta» o «Recibiendo respuesta», sin afirmar «Razonando». Streaming se agrupa por `requestAnimationFrame`, con vaciado final y anuncio al completar. Autoscroll solo si se sigue respuesta (menos de 96px del final); «Ir al último mensaje» recupera seguimiento. «Herramientas utilizadas» despliega nombres recibidos por eventos reales `mcp_used`; no representa inicio/fin ni progreso de ejecución que el stream no proporciona.

### Workspaces, recursos y ajustes
`Workspace` es una sección no modal junto a navegación: foco inicial en «Volver al chat», sin trampa de Tab ni scrim; al desmontarse devuelve foco al disparador visible o menú disponible. Escape vuelve al chat, salvo que un diálogo nativo abierto gestione la tecla. Chat conserva su estado mientras está oculto e inert. Drawer móvil abierto sigue siendo modal con `useSheetFocus`; cerrado queda invisible para interacción. Documentación y voz mantienen sus capas propias.

`ServicesSheet` presenta Todas, Servidores MCP, Bases de datos y Sistema de Ira. Navegación por tarea mantiene controles de mínimo 44px, texto muted en reposo y acento con tinte al seleccionar. `hidden` conserva componentes montados, borradores y resultados al cambiar filtros; Ajustes hace lo mismo entre sus secciones. Esta conservación vale dentro del workspace montado, no después de cerrarlo o navegar a otro. Recursos comparten lista, nombre, descripción, `Status` textual y acción contextual; errores junto al recurso, detalles y eliminación progresivos con confirmación. Habilitado significa disponible para Ira, no acceso comprobado.

Sistema de Ira separa almacenamiento interno de bases consultables. Estado distingue Disponible, Iniciado · no disponible y Detenido. Inicio/parada quedan a mano; diagnóstico, arranque automático, puertos y edición de nombre/descripción con «Guardar detalles» aparecen bajo detalles. Puerto del gateway también es avanzado; no se exige conocer puertos en el flujo principal.

`Catalog` organiza Proveedores, Comportamiento, Herramientas y Apariencia. Proveedores e instrucciones usan guardar/cancelar explícitos; URL base y eliminación son avanzados. Presencia de clave guardada/del entorno o cuenta autorizada no acredita acceso verificado. Guardado de proveedor puede ser parcial entre operaciones; el error lo declara. Búsqueda web y permisos se guardan al cambiar con feedback de guardando/éxito/error; tema se recuerda en el dispositivo.

### PostgreSQL e integraciones MCP
`Databases` permite pegar URL PostgreSQL y «Guardar y comprobar» directamente, o «Revisar datos de URL» para importar un resumen y editar campos; entrada manual y puerto/SSL son progresivos. Una URL nueva sustituye los datos importados anteriores al guardar. `parseDatabaseUrl` interpreta localmente `postgres://`/`postgresql://`: decodifica credenciales, admite IPv6, puerto válido y solo `sslmode` reconocido; rechaza fragmentos y opciones desconocidas sin repetir la URL en errores ni hacer logging. URL y contraseña son estado React transitorio, sin persistencia frontend. Importar vacía URL; guardar transmite campos estructurados, limpia contraseña del borrador y pasa a edición de la conexión guardada. Contraseña vacía en edición conserva la guardada.

«Guardar y comprobar» distingue `saving` y `testing`, guarda primero y prueba por ID. Error de guardado mantiene datos introducidos; fallo de prueba mantiene editor y conexión guardada para corregir/reintentar, éxito cierra. Foco entra al primer campo, vuelve al editor tras fallo de comprobación y al disparador al cancelar/cerrar con éxito. Campo inválido dentro de detalles los abre. La lista distingue Sin comprobar, Última prueba correcta, Necesita atención y Desactivada; resultado de prueba no promete disponibilidad futura.

`McpConnections` usa URL remota o ejecutable con argumentos ordenados; headers y variables de entorno se editan como pares nombre/valor, con validación de nombres vacíos/repetidos. JSON solo importa configuración avanzada, no es requisito del flujo. Guardar comprueba automáticamente una integración habilitada: éxito cierra, fallo conserva borrador y editor, con foco para corregir; una integración desactivada se guarda sin prueba. Cancelar o cerrar con éxito restaura foco al disparador. Estado distingue activación de comprobación y muestra cantidad/lista real de herramientas descubiertas; una región viva persistente anuncia comprobando/éxito/fallo, incluido resultado de cero herramientas.

Alcance MVP: no incorpora nuevas recetas ni OAuth nativo para MCP. La opción SSE/OAuth usa puente local y requiere Node/npx; no equivale a autorización nativa dentro de Ira. El flujo ChatGPT/Codex existente sigue siendo autorización externa. No se añaden eventos de inicio/fin de herramientas.

### Selector de modelo
`ModelPicker` monta `<dialog>` mediante portal a `document.body` y `showModal()`: capa fuera del scroll, foco y contención nativos. Búsqueda por nombre de modelo o proveedor, modelo activo primero y luego proveedores configurados; modelos con clave «falta» deshabilitados y explicación visible. Esta prioridad usa presencia de credenciales, no una prueba al proveedor. Flechas recorren opciones habilitadas; Escape/cierre o elección devuelve foco al disparador. Abre con foco en búsqueda, limita cierre durante cambio pendiente, conserva error local y ofrece acceso a Proveedores.

### Movimiento
`MotionConfig reducedMotion="user"`: transición general de 220ms con `cubic-bezier(0.16, 1, 0.3, 1)`. Workspace entra 200ms con desplazamiento horizontal de 10px y opacidad; Message entra una vez en 200ms con el mismo easing. Animar navegación/entrada, no lectura ni cada delta. CSS de controles usa 150–280ms; órbita de actividad usa 1.6s linear en espera y 0.9s al recibir.

Sidebar usa transición de width/padding de 420ms con `cubic-bezier(0.22, 1, 0.36, 1)`, coordinada con marca, botón y distribución de conversaciones; desactivada durante arrastre por puntero y con movimiento reducido. En móvil se usa transform de 200ms ease. No extender esta excepción a otros cambios de tamaño.

`prefers-reduced-motion: reduce` anula animaciones/transiciones CSS y scroll suave; Motion respeta preferencia, mensajes omiten entrada y dock omite layout. Indicador orbital queda estático; el texto sigue distinguiendo espera y recepción.

### Evidencia de cierre
Handoff recibido: disposición del reviewer **ship**, limitada a cinco fixes puntuados: URL editada sustituye importación anterior, conservar draft MCP tras fallo, anclaje del selector según alto real, anuncios de comprobación y gestión/restauración de foco. Este veredicto no constituye aprobación global de la superficie.

Validación funcional reportada en el handoff: 16 pruebas Playwright aprobadas mediante `npm test` y `npm run build` aprobado; usan mocks, sin acceso real a PostgreSQL ni proveedores. Detector ejecutado una vez, resultado `[]`. No se repiten estas comprobaciones en el pase documental.

Capturas referenciadas por el handoff: `.impeccable/review/{desktop-connections,mobile-connections,desktop-settings,mobile-settings,desktop-model-picker,mobile-model-picker,desktop-database,mobile-database,desktop,desktop-chat,mobile,mobile-drawer}.png`. Este pase contrasta documentos con fuente vigente; no produce ni revalida capturas. Validar JSON y diff focalizado al actualizar estos dos documentos.

## Do's and Don'ts

### Do:
- **Do** conservar logo existente, Hugeicons y etiquetas accesibles.
- **Do** reutilizar Field y roles de color en ambos temas.
- **Do** distinguir guardar de comprobar y mantener lectura quieta durante streaming.
- **Do** conservar borradores entre filtros y tras fallos, con resultado local y retorno de foco.

### Don't:
- **Don't** recuperar la estética Grok monocroma ni las antiguas franjas iridiscentes.
- **Don't** persistir URL o credenciales crudas en almacenamiento frontend.
- **Don't** convertir movimiento continuo o sombras en decoración de cada mensaje.
- **Don't** presentar credenciales presentes como acceso verificado ni inventar fases de herramientas.
