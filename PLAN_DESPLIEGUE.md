# Plan de despliegue de Ira

## Objetivo

Acceder a Ira desde móvil y otros dispositivos mediante `https://ira.markusdev.me`, con la aplicación y los datos en el ordenador de casa. No usar Tailscale, Cloudflare Zero Trust ni un plan que requiera tarjeta. Mantener intactos `markusdev.me` y los subdominios de las otras aplicaciones.

## Estado actual

- `ira-server` y la interfaz web funcionan con Docker Compose; PostgreSQL y el resto de servicios también están en Compose.
- La web tiene login con un token técnico y una sesión en cookie. El servidor publica el puerto `8787` únicamente en `127.0.0.1`.
- Cloudflare gestiona los DNS del dominio; los registros existentes permanecen en modo **DNS only**.
- Existe una configuración opcional para Cloudflare Tunnel en `deploy/compose.remote.yml`. Este plan propone otra vía de publicación.

## 1. Comprobar viabilidad de la conexión doméstica

Antes de cambiar la red o escribir la integración de publicación:

1. Comprobar si el router recibe una IP pública y si el operador utiliza CG-NAT.
2. Confirmar acceso a la configuración del router y disponibilidad de los puertos entrantes `80` y `443`.
3. Comprobar si la IP pública es fija o dinámica.

**Criterio de decisión:** si hay IP pública y se pueden abrir ambos puertos, continuar con Caddy en casa. Si hay CG-NAT o los puertos no están disponibles, detener esta vía y diseñar una alternativa con servidor intermedio o túnel. Un demonio por sí solo no elimina CG-NAT.

## 2. Login personal

Separar la credencial humana de `IRA_HTTP_TOKEN`, que seguirá siendo la credencial técnica para clientes y servicios internos:

1. Sustituir el campo «Token de acceso» por un login con contraseña de una sola cuenta personal, utilizable en varios dispositivos. No añadir registro público de usuarios.
2. Configurar la contraseña fuera del frontend y almacenarla como hash Argon2id; no guardar la contraseña en texto claro ni en el repositorio.
3. Mantener sesiones independientes por dispositivo con cookies `HttpOnly`, `Secure` y `SameSite`; añadir cierre de sesión y revocación de la sesión correspondiente.
4. Limitar intentos de login y proteger las operaciones de escritura frente a peticiones de otro origen. Evitar que el frontend incorpore `IRA_HTTP_TOKEN` en el bundle.
5. Decidir explícitamente la duración de sesión y si debe sobrevivir a reinicios; adaptar el almacenamiento de sesiones a esa decisión.

**Verificación:** contraseña incorrecta denegada, login correcto desde dos dispositivos, logout de uno sin afectar al otro, API inaccesible sin sesión y sin token técnico.

## 3. Arranque automático

1. Configurar Docker para iniciar al arrancar el ordenador.
2. Aprovechar las políticas `restart: unless-stopped` del Compose actual; usar una unidad `systemd` para levantar el proyecto Compose solo si hace falta para garantizar el arranque.
3. Evitar instancias duplicadas y comprobar que Ira vuelve a estar sana tras reiniciar el equipo.

El ordenador tendrá que permanecer encendido y conectado a Internet mientras se quiera usar Ira desde fuera de casa.

## 4. DNS, HTTPS y publicación

1. Crear en Cloudflare únicamente un registro `A` para `ira.markusdev.me` apuntando a la IP pública de casa. Mantener los registros de las otras aplicaciones sin cambios.
2. Si la IP cambia, configurar actualización automática únicamente de ese registro mediante un token DNS de alcance mínimo.
3. Instalar Caddy como proxy inverso con el hostname `ira.markusdev.me`, certificado HTTPS automático y destino interno `ira-server:8787`.
4. Publicar hacia Caddy solo los puertos `80` y `443` del router; mantener `ira-server`, PostgreSQL, Toolbox y el gateway sin puertos públicos directos.
5. Activar cookies `Secure` detrás del proxy y verificar que las peticiones llegan con el host correcto para las comprobaciones de origen.

Cloudflare se usaría aquí para **DNS**, sin Cloudflare Access ni túnel. La aplicación seguiría protegida por su propio login.

## 5. Pruebas y operación

- Probar `https://ira.markusdev.me` desde el móvil con **datos móviles**, no solo con Wi-Fi.
- Comprobar emisión y renovación del certificado, login, logout, peticiones API y reinicio del ordenador.
- Confirmar que `markusdev.me` y los subdominios existentes siguen funcionando.
- Preparar copias de seguridad del volumen PostgreSQL, `.ira/master.key` y `services/projects-api/data/`; comprobar una restauración.
- Documentar configuración, actualización y diagnóstico del servicio para poder operarlo sin iniciar sesiones manuales en el ordenador.

## Alcance inicial

Primera entrega: chat y panel web. La voz en tiempo real necesita una ruta HTTPS/WebSocket autenticada y ajustes del cliente web; queda para una fase posterior.
