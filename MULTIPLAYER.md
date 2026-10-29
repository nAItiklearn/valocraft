# Multiplayer in Valocraft

Valocraft has **no central account service and no connection to any external
game infrastructure**. Every server is independent.

## Playing multiplayer

1. Get the server's address from whoever runs it (e.g. `play.example.com:14004`
   or an IP — the default game port is **14004**).
2. In the client, log out to the main menu, go to **Multiplayer**, and add the
   server address.
3. Pick a username and play. **No account, no registration, no sign-up.**

## Hosting a server

Every desktop download includes the dedicated server
(`ValocraftServer`, `run-server.bat`, or `run-server.sh`):

1. Start it once — it creates `userdata/server/` with configuration and saves.
2. Friends on your LAN can connect immediately. For internet play, forward
   port 14004 (TCP/UDP) on your router/firewall and share your public address.
3. Server rules, plugins, and gameplay settings live in the server config
   files under `userdata/server/`.

## Optional: accounts on your own server

Servers are account-free by default. If you want players to authenticate,
Valocraft ships its own self-hostable auth server in `libs/auth/server`
(it is built into this repository — no external services involved):

1. Build and run the auth server on your own machine or VPS.
2. Set its address in the game server settings (`auth_server_address`).
3. Players register on *your* auth server and trust only *your* endpoint
   (client: Settings → Network → trusted auth servers).

Nothing in Valocraft — client, server, or auth — contacts any third-party
service. You own the whole stack.
