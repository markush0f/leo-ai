# Leo desktop

Ventana de chat y catálogo. Mismo Postgres y mismas tools que `leo`.

```
docker compose up -d
cd desktop
npm install
npm run tauri dev
```

Solo la UI (sin LLM, datos de muestra): `npm run dev` (puerto 5179; si quedó un Vite colgado, `npm run dev` lo mata y lo reutiliza).

En NVIDIA + Wayland el binario fuerza `WEBKIT_DISABLE_DMABUF_RENDERER=1` y `GDK_BACKEND=x11` para evitar el *Error 71* de WebKitGTK. Si quieres Wayland puro: `GDK_BACKEND=wayland npm run tauri dev`.
