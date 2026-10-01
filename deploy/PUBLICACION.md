# Publicar Ira en casa

## Recomendado: router + Caddy + `ira.markus.dev`

Esta es ruta recomendada si router tiene IPv4 pública y deja reenviar 80/443. **No actives túnel Serveo ni Compose túnel** al usar esta vía. Los pasos completos están en secciones «Requisito de red», «Contraseña», «DNS, HTTPS y arranque» y «Verificación» inferiores. Dominio `markus.dev` usa DNS Porkbun; `ira.markus.dev` actualmente apunta a `165.22.204.233` (solo cambiar registro `ira`). Si WAN no coincide con IP pública o puertos están bloqueados, usa alternativa de túnel indicada debajo.

## Dominio fijo sin router: `ira.markus.dev` con Serveo

**Dominio distinto a `ira.markusdev.me`.** `markus.dev` usa DNS Porkbun y `ira.markus.dev` ya tiene un registro A hacia `165.22.204.233`. Hace falta acceso DNS de `markus.dev` para sustituir **solo** ese registro; no tocar otros registros. Este túnel saliente no requiere abrir puertos ni IP pública en casa. Serveo es un tercero con disponibilidad no garantizada y muestra pantalla de aviso en plan gratuito. Consulta condiciones de dominio propio en https://serveo.net/docs/ antes de depender del servicio.

1. Crea hash de contraseña siguiendo bloque «Contraseña y sesión» inferior. Arranca Ira con cookies seguras: `docker compose -f docker-compose.yml -f deploy/compose.tunnel.yml up -d --build ira-server`.
2. Crea clave exclusiva para túnel y obtiene huella pública:

   ```sh
   ssh-keygen -t ed25519 -f ~/.ssh/ira_serveo -N ''
   ssh-keygen -lf ~/.ssh/ira_serveo.pub
   ```

3. En panel DNS **Porkbun** de `markus.dev`: borra únicamente registro `A ira` existente; crea `CNAME ira → serveo.net` y `TXT _serveo-authkey.ira → SHA256:...` (solo huella copiada del comando anterior, sin prefijo `256` ni resto de línea). Espera propagación y comprueba con `dig +short CNAME ira.markus.dev` y `dig +short TXT _serveo-authkey.ira.markus.dev`. Nunca copies clave privada a DNS.
4. Prueba túnel manual desde PC, aceptando clave SSH del servidor solo si fingerprint coincide con el publicado en https://serveo.net/docs/:

   ```sh
   ssh -i ~/.ssh/ira_serveo -o IdentitiesOnly=yes -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 -R ira.markus.dev:80:localhost:8787 serveo.net
   ```

   Comprueba `https://ira.markus.dev` desde móvil con datos. Si Serveo deniega dominio (por disponibilidad o condiciones del plan), no cambies otros DNS para improvisar: restaura el registro A anterior y elige otra vía.
5. Tras probar acceso, cierra túnel manual (`Ctrl+C`) y activa servicio de usuario con reconexión automática:

   ```sh
   mkdir -p ~/.config/systemd/user
   cp deploy/ira-serveo.service ~/.config/systemd/user/
   systemctl --user daemon-reload
   systemctl --user enable --now ira-serveo.service
   sudo loginctl enable-linger "$USER"
   systemctl --user status ira-serveo.service
   ```

   Actualizaciones: usa mismo Compose `-f docker-compose.yml -f deploy/compose.tunnel.yml` y revisa `journalctl --user -u ira-serveo.service`. No lances simultáneamente otra conexión usando mismo dominio.

## Sin acceso al router: enlace temporal

Si no puedes abrir puertos, genera primero hash de contraseña siguiendo sección siguiente. Arranca servidor con cookies HTTPS y abre túnel gratuito desde una terminal del PC:

```sh
docker compose -f docker-compose.yml -f deploy/compose.tunnel.yml up -d --build ira-server
ssh -o ServerAliveInterval=30 -R 80:localhost:8787 nokey@localhost.run
```

Abre desde móvil con datos la URL `https://…localhost.run` indicada por SSH. Mantén terminal y PC encendidos. URL gratuita cambia periódicamente; no es `ira.markus.dev` ni acceso directo por IP. No ejecutes Compose público al mismo tiempo: esa variante publica 80/443 del host y necesita configuración del router.

## Requisito de red (antes de activar publicación)

1. Abre `http://192.168.1.1` desde PC y compara IPv4 WAN indicada por router con `curl -4 https://api.ipify.org` (en comprobación inicial PC mostró `88.24.136.121`; puede cambiar). Si WAN cae en `10/8`, `172.16/12`, `192.168/16` o `100.64/10`, o no coincide con IP pública, solicita IP pública al operador: CG-NAT impide esta vía.
2. Confirma disponibilidad de TCP 80 y 443. Configura reserva DHCP para PC (`192.168.1.35` ahora; verificar en router) y reenvía **solo** esos puertos TCP a esa IP, externos 80→interno 80 y 443→interno 443. No reenvíes 8787, 5432, 5000 ni 8790. Abre firewall del PC con `sudo firewall-cmd --permanent --add-service=http --add-service=https && sudo firewall-cmd --reload`. Confirma desde red externa (datos móviles); pruebas desde Wi-Fi pueden fallar por falta de NAT loopback.
3. Si no hay IP pública o ambos puertos no están disponibles, detén esta vía. Hace falta túnel o servidor intermedio; ningún proceso local resuelve CG-NAT.

## Contraseña y sesión

Genera hash localmente; contraseña no debe escribirse en argumentos, `.env`, repositorio ni historial. Desde raíz del proyecto:

```sh
mkdir -p .ira
HASH_FILE=$(mktemp)
read -rs -p 'Contraseña Ira: ' IRA_PASSWORD; printf '\n'
printf '%s' "$IRA_PASSWORD" | cargo run --quiet -p ira-server --bin ira-password-hash > "$HASH_FILE"
unset IRA_PASSWORD
sudo install -m 600 -o 10001 -g 10001 "$HASH_FILE" .ira/password.hash
rm "$HASH_FILE"
```

Compose (local y público) exige `IRA_PASSWORD_HASH_FILE=/home/ira/.ira/password.hash`; prepara archivo antes de arrancarlo. `ira-data-permissions` ajusta propietario UID 10001. Token técnico `IRA_HTTP_TOKEN` (o `.ira/http.token`) sigue para clientes internos; jamás se envía al navegador. Rotar contraseña: sustituir hash, reiniciar `ira-server` (invalida sesiones actuales). Cookies `HttpOnly`, `SameSite=Strict` y `Secure` al activar Compose público; sesiones independientes por dispositivo, expiran a las **24 horas** y se invalidan al reiniciar servidor (memoria). Cerrar sesión revoca únicamente cookie actual. Cinco intentos fallidos en cinco minutos bloquean login globalmente durante ventana restante.

## DNS, HTTPS y arranque

En Porkbun cambia **solo** registro A `ira.markus.dev` (subdominio `ira`) de `165.22.204.233` a IP pública de casa (`curl -4 https://api.ipify.org`). No toques `markus.dev` ni otros registros. Si IP cambia, crea par de claves API Porkbun restringidas al dominio `markus.dev` en https://porkbun.com/account/api y guarda JSON `{"apikey":"...","secretapikey":"..."}` en `.ira/porkbun-dns.json` (propietario UID 10001, modo 600). API restringe por dominio, no por registro; script solo edita registro A existente de `ira.markus.dev`, cada cinco minutos. No habilites restricción por IP en clave si IP cambia. Activa perfil:

```sh
docker compose -f docker-compose.yml -f deploy/compose.public.yml --profile dynamic-dns up -d --build
```

Para IP fija, omite `--profile dynamic-dns`:

```sh
docker compose -f docker-compose.yml -f deploy/compose.public.yml up -d --build
```

Caddy termina HTTPS automáticamente y reenvía a `ira-server:8787` conservando Host; no publiques otros puertos del router. El servidor activa cookies Secure en esta configuración. Los puertos auxiliares de Compose están ligados a `127.0.0.1` del host. No uses simultáneamente `deploy/compose.remote.yml`.

Activa Docker al arrancar (`sudo systemctl enable --now docker`). Compose utiliza `restart: unless-stopped`; el proyecto se levanta una vez con `up -d`, Docker lo reinicia tras arranque normal sin unidad systemd adicional. Si algún servicio fue parado manualmente, levántalo con `up -d` (evita crear otro proyecto con distinto `-p` o directorio).

## Verificación y mantenimiento

- Desde móvil con **datos móviles** abre `https://ira.markus.dev`. Comprueba candado HTTPS y login erróneo/correcto, segunda sesión en otro dispositivo, cierre de sesión sin afectar segunda sesión y API `/api/snapshot` sin cookie → 401.
- `docker compose -f docker-compose.yml -f deploy/compose.public.yml ps` muestra estado; `logs ira-public ira-server ira-dns` ayuda a diagnosticar certificados, login y DNS. `curl -I https://ira.markus.dev` confirma HTTPS; comprueba renovación mediante logs de Caddy tras tiempo y reinicio del ordenador. Verifica también `markus.dev`, `markusdev.me` y subdominios existentes.
- Actualiza con mismo comando `up -d --build` tras actualizar código; no elimines volúmenes (`down -v` borra datos). Si necesitas renovar contraseña, regenera hash y `docker compose -f docker-compose.yml -f deploy/compose.public.yml restart ira-server`.

### Copia y restauración

Con servicios activos, guarda dump y archivos en lugar privado fuera del repo; protege y cifra destino. Usa mismo nombre de proyecto Compose durante restauración:

```sh
docker compose -f docker-compose.yml -f deploy/compose.public.yml exec -T postgres pg_dump -U ira -Fc ira > ira.dump
cp .ira/master.key master.key.backup
cp -a services/projects-api/data projects-data.backup
```

Para probar restauración sin pisar producción, crea base temporal en mismo PostgreSQL, restaura y consulta; elimínala después:

```sh
docker compose -f docker-compose.yml -f deploy/compose.public.yml exec -T postgres createdb -U ira ira_restore_test
docker compose -f docker-compose.yml -f deploy/compose.public.yml exec -T postgres pg_restore -U ira -d ira_restore_test --no-owner < ira.dump
docker compose -f docker-compose.yml -f deploy/compose.public.yml exec -T postgres psql -U ira -d ira_restore_test -c 'SELECT count(*) FROM conversations'
docker compose -f docker-compose.yml -f deploy/compose.public.yml exec -T postgres dropdb -U ira ira_restore_test
```

Para recuperación real, detener escritores y restaurar dump en base vacía, `master.key` y `projects-data.backup` con permisos correctos; no generar nueva clave antes de restaurar la anterior. Primera entrega solo chat/panel; voz web remota requiere autenticación WebSocket y ruta HTTPS propia.
