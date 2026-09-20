# frontend-web

The web frontend of Alice Voice. The package is a Qwik City application
with a transparent heads-up display. The display holds the conversation
with the daemon and shows the daemon state through the aura.

The project is separate from `backend/` and from `frontend-desktop/`.
It talks to the daemon through REST and one WebSocket only. See
[[../guidelines/frontend/OVERVIEW]] for the boundary rules.

## Commands

```sh
npm install          # install the dependencies
npm start            # start the dev server
npm run build        # type check, lint, and build
npm run lint         # run eslint on src
npm run fmt          # format with prettier
npm run fmt.check    # check the formatting
npm run build.types  # run tsc --noEmit
npm run test         # run the unit tests with vitest
npm run test:e2e     # run the Playwright end-to-end suite
```

## End-to-end tests

The suite drives the queue toggle flow in a real browser against a live
backend and frontend. `playwright.config.ts` starts both through its
`webServer`: the daemon, and a `vite preview` build of the app. No manual
setup is needed beyond Rust and the Playwright browsers.

```sh
cargo build -p alice-daemon   # optional: warm the target dir first
npm run test:e2e              # chromium, headless
npm run test:e2e:ui           # watch mode
npm run test:e2e:debug        # step through
```

The suite is self-contained by default: the daemon runs on a sqlite file at
`.tmp/e2e.db`, so Docker is not required. It writes real messages and flips
`app_setting`, so it must **never** run against the live database.

The suite always starts its own daemon and its own preview server. It never
reuses a running server, and its daemon listens on port `8790`, not the dev
port `8788`. A reused dev daemon would ignore `DATABASE_URL` and write to
the live database. The config also refuses a Postgres `DATABASE_URL` that
names the live `alice` database. Use the isolated stack instead:

```sh
# Start the isolated test database (name alice_e2e, port 5435)
docker compose --env-file docker/.env -f docker/compose.test.yml up -d
DATABASE_URL=postgres://alice:alice@127.0.0.1:5435/alice_e2e npm run test:e2e
```

| Var                   | Default                              | Use                          |
| --------------------- | ------------------------------------ | ---------------------------- | --- | ------------ | ------ | ----------------------------------------- |
| `DATABASE_URL`        | `sqlite:<repo>/.tmp/e2e.db?mode=rwc` | Isolated database under test |     | `ALICE_PORT` | `8790` | Daemon port (own port, never the dev one) |
| `WEB_PORT`            | `3000`                               | Preview server port          |
| `PLAYWRIGHT_BASE_URL` | `http://127.0.0.1:<WEB_PORT>`        | `baseURL` of the tests       |

The tests flip the real `app_setting` row and restore it when they finish,
so a run leaves the queue where it found it. A failed run writes a
screenshot, a video, and a trace to `test-results/`.

## Backend connection

The daemon listens on `http://127.0.0.1:8787` by default. Set
`PUBLIC_ALICE_API_URL` to point the client at another host. The value is
read at build time.

```
PUBLIC_ALICE_API_URL=http://127.0.0.1:8787
```

The REST calls run on the server inside `routeLoader$` and `server$`.
Only the socket client runs in the browser.

| Endpoint                                           | Use                                     |
| -------------------------------------------------- | --------------------------------------- |
| `GET /api/v1/status`                               | The first status snapshot of the shell  |
| `GET /api/v1/conversations`                        | Every stored conversation, newest first |
| `GET /api/v1/conversations/{id}`                   | One conversation with its full history  |
| `POST /api/v1/chat`                                | Store one turn in a conversation        |
| `GET /api/v1/resolver`                             | The engine, the models, and the budget  |
| `POST /api/v1/resolver/preview`                    | Read one sentence and report its route  |
| `POST /api/v1/resolver/local/models/{id}/download` | Install a built in model                |
| `DELETE /api/v1/resolver/local/models/{id}`        | Remove a built in model                 |
| `WS /api/v1/events`                                | The live event stream                   |

## Conversations

Every message belongs to a conversation, and a conversation owns its
full message history. The daemon builds the title from the first message
of the conversation, so a title is never typed by hand.

The daemon writes a turn in four steps. It resolves the conversation,
puts the user message in the intake queue, handles the message, and moves
the handled messages into the conversation. Handling resolves the intent
of the message, runs the command of that intent, and answers with the
output of the command. The queue toggle decides whether the daemon stores
the turn at all: with the queue off it answers without a store and
reports `stored: false`.

The routes follow the model:

- `/` continues the most recent conversation and shows its history.
- `/?new=1` starts an empty surface. The first message creates the conversation.
- `/conversations` lists every stored conversation.
- `/conversations/{id}` shows one conversation with its full history.

The client starts without the daemon. The shell then shows the offline
state and the socket client keeps trying to reconnect.

## Conversation surface

The surface is two places side by side.
The transcript panel takes the right side and scrolls on its own, so a
new turn never moves the stage.
The stage keeps the aura in the middle, the composer at the bottom, and
the stages of the running turn right above the composer, so the work of the
daemon reads as the answer that is on its way.
A narrow window stacks the panel above the stage.

The transcript follows the newest turn while the reader is at the end of
the region. A reader who scrolled up keeps their place.

The turn of the user carries the accent veil, and the turn of the daemon
carries plain glass. The header line of a turn keeps the time and how long
its command took.

## Turn metadata

A turn reports how the daemon read it, so a stored message explains
itself. `ChatMessageDto.meta` carries the engine that chose the intent
with the model it ran, the engine that read the entity values with its
model, the entities with the values they were given, the rendered
command, and the exit code and duration of the run. The daemon stores the
object in the `chat_message.meta` JSON column and streams the same fields
while the turn runs.

A turn read by the router also carries its **route**: one entry per stage
the message passed, in the order it ran them, with the stage name, how the
stage ended (`answered the turn`, `refused the message`, `passed the turn
on`, `fell back to a cheaper reader`, `did not run`), the reader it ran
with its model, one sentence about what it read, and how long it took. So
a reader sees that a message ran through every stage, that the
deterministic pass answered one outright, or that a stage wanted a reader
that did not answer and ran its fallback instead. A refusal carries its
reason, for example that the best two intents scored closer than the
margin.

Every value carries the reader that read it: `name = value  list` is a
value the rules of a list matched, `spans` is one the built in model found,
`model` is one a language model guessed, and `proved` is one the
deterministic pass read out of the message alone. What the reader read
before the daemon matched it to an entry of a list follows it, so a name a
model read next to the entry it reached (`applications = firefox  list
(read firefox browser)`) is on screen and not lost behind the match.

The engines are the ones that really read the turn and not the ones the
settings allow: a hybrid turn that fell back to the language model for
its values reports `llama` for both steps.

A turn the resolver read and chose no intent for still reports that read:
the table names the engine under `Resolver` and reads `no intent matched`
where the name of the intent would sit, so the work of the daemon is on
screen whatever it answered.

The transcript shows the report on the daemon turn of a turn, so the user
turn above it stays text. The header line of a turn keeps the name and the
time: the time carries a `Tooltip` with the length of the run, the
`Resolver` row carries a `Tooltip` with the models the engines ran, and
the rest waits behind the `Metadata` toggle of the turn, one field per row
of a small table. The transcript stays quiet until a reader asks.

The composer takes the caret when the surface loads and again when the
answer of the daemon lands, so the next message needs no click.

## Intent resolver

The layered router reads the intent of a message in stages and stops at
the first stage that can answer. It reads a catalog of any size, because
only the short list of one turn reaches a model, and every stage has a
reader that needs no model, so a server that is down costs accuracy rather
than the turn. The settings page configures that router and nothing else:
the engine is not a choice, so a stage carries its own reader instead.

A GLiNER model comes from Hugging Face, and the page downloads it, shows
its progress, and removes it again. The model runs on the processor, and
on a CUDA capable graphics card when the daemon is built with
`--features gliner-cuda`. The root `shell.nix` provides the CUDA libraries
for that build, and `.envrc` loads the shell with direnv.

### The layered router

The panel draws the router as a node graph, the way a node editor does:
one block per step, joined by the value that moves along the link (`one
message`, `short list of 8`, `one intent`, `the values`), so a reader sees
what leaves a block and what the next block receives.

The graph is a canvas. The first view fits the whole graph into the card,
a drag moves it, and the wheel or the `−`, `+` and `Fit` controls change
the zoom, so a graph wider than the card needs no scrollbar of its own and
a reader who wants the detail of one block zooms in instead of losing the
overview. A drag never picks a block, so a reader can move the graph with
out changing what they are editing.

The three kinds of block stand in three columns, so every link runs
forward and a reader never walks a link backwards: the places a stage
reads in the first column, the chain in the middle, and the exits in the
last one. An exit sits on the row of the stage it leaves, because stacking
the exits below the chain would cross the link of one exit with the other:
the decision leaves above the extraction and refuses below it.

Three kinds of block share the canvas.

- **A stage** is one step of the chain, numbered from the message to the
  run. It is a filled block.
- **A place** is somewhere a stage reads: the model server, the built in
  model files, the built in GLiNER. It is a sunken block, wired to every
  stage that reads it, and the link names what it delivers (`embeddings`,
  `chat`, `reranker`, `spans`). A place is drawn only while a stage really
  reads it, so the graph never shows a server or a model file a turn would
  not touch.
- **An exit** is a turn that leaves the chain: `No intent matched`, and
  `Asks for the value`. It is a dashed block.

A link of the chain is solid. A link to a place or to an exit is dashed,
so the chain the message follows reads apart from what it reads and from
where it can leave.

The graph reads one route at a time when the user tried a sentence: the
blocks and links the message reached keep their color and everything else
fades, so one press shows the way a message really went instead of the
whole pipeline at once. `src/utils/router-route.ts` turns the route of a
preview into the ids of those blocks and links and the tone of each one: a
stage that answered is green, a stage that fell back to another reader is
amber, and the stage that refused is red, and a link carries the tone of
the block it leads into. A stage that fell back also lights the place of
the reader it really ran.

The graph is read only. Picking a block opens its settings beside the
graph, in the words of that block: a stage carries its readers, the
`Model server` block carries the address and the chat model, and the
`Built in GLiNER` block carries the model on disk, the device, and the
threshold. One block is on screen at a time, so the panel stays short and
every value stays editable where the graph says it belongs.

Every stage is chosen on its own and can be opted out, so the pipeline is
as small or as thorough as the machine wants it.

- **The message** takes the text of one message with the turns before it.
  It is also where a sentence is tried: the panel of this block holds the
  sentences the user really says, plays one of them, or plays all of them.
  A sentence is read with the stored settings and runs nothing: no turn,
  no message, and no command comes of it. The sentences themselves are
  stored (`preview_sentences`), because they are the tests of the
  pipeline: a user who tuned the router against a sentence finds it again
  after a reload, and the list survives the page that wrote it, and every
  sentence can be edited in place. A sentence keeps its own log, so a run
  of all of them is a run of many answers: pressing a sentence shows the
  log of that sentence, and the run ends on the one it read last. The log
  reads in two ways, and the switch above it picks the way: **Text** says
  what the daemon would do in the words of a user, and **Debug** names
  every value the daemon reported, field by field, with a hint beside each
  of them (`src/utils/preview-log.ts`). Debug colors what is worth
  finding: the answer, the color of every stage by the way it ended, and a
  value the daemon read nothing for. The graph then keeps the route of the
  shown sentence, colors it, and steps every stage and place it did not
  meet back, and `Show every stage` returns the whole graph.
- **Deterministic pass** proves a message the catalog can prove: an intent
  the message spells out, or one value of one list that only one intent
  owns. It needs no model and never guesses.
- **Retrieval** ranks every intent of the catalog against the message and
  keeps the short list. It reads the **words** of the catalog, the
  **embeddings**, or **both**. The words need no model at all.
- **Decision** chooses one of the short list. The **scores** read the
  ranking alone and need no model, the **reranker** reads the message and
  every candidate with a built in cross-encoder, and the **model** reads
  only the short list, so the prompt stays small however large the catalog
  is.
- **Extraction** reads the values of the intent that won. The **lists**
  need no model, the **spans** use the built in model, and the **model**
  reads every value.
- **The run** runs the command. A required value that is missing stops the
  turn and asks the user for it.

The badge of a block names the reader it would run right now, and a place
reports whether it answers (`Up`, `Down`, `On disk`, `Missing`), so the
state of the whole pipeline is on the graph without a reload. A stage that
reads embeddings while its place does not answer takes the warning tone.

The address of the server is one database setting, so every stage that
reads it and the `Model server` block write the same value.

The flow takes the room the section leaves rather than a height of its own,
and the panel of the picked block stands beside it at the same height with
its own scroll and the save of the resolver in its footer, so a reader who
changed a stage at the top of a long list reaches the save without
scrolling the page. The panel grows with the screen from 22rem to twice
that, so a wide screen gives the values the room to read.

The weights fold the words and the embeddings into one score, and a weight
of zero drops that evidence. The floor and the margin gate the ranking:
with the scores or the reranker, a turn whose best score stays below the
floor or whose best two stay closer than the margin refuses before a model
reads. A generative decision stage is not gated that way, because it
refuses with its own "none of these" option instead.

Embeddings come from the model server (the `/embeddings` path of the
address above, no download) or from a built in ONNX model that runs in the
daemon with no server and no network after the download. The two built in
models are an embedding model and a reranker, and the page shows only the
roles the current settings read. A model is a file on disk, so the row
says whether it is there, and the page downloads it, polls its progress,
and removes it again. The device follows the GLiNER choice: the processor,
or a CUDA capable graphics card when the daemon is built for it.

A built in model runs through ONNX Runtime (`ort`), so the daemon needs no
second process and the models share the device list of GLiNER. A device
the build or the machine does not offer is not shown.

GLiNER reads the message and the labels in one sequence, so a large
intent list costs accuracy. The page shows the label count and warns past
20 labels, and past 30 it suggests another reader for the values. A catalog that reads
more than 20 labels offers the names of its intents and the labels of
their entities only, and leaves the phrases to the word reader: a phrase
names its own intent, but the value inside it — `close firefox` holds
`firefox` — names the intent that acts on the same value as well, so the
phrases of two intents that read the same kind of value read each other's
messages. A small catalog keeps the phrases, and every catalog keeps them
for the word reader either way.

## Intent configuration

The settings page writes one intent at a time. A row is the pick itself,
so a reader presses an intent to open its values in the panel beside the
list rather than reaching for an edit button, and the delete keeps its own
press in the row. The list takes the rest of the section and scrolls inside
it, and the panel of the picked intent stands at the same height with its
own scroll. A fresh catalog seeds 24
example intents, so a new installation answers the common things at once:

- **An application** — `open application` opens a program by name from
  the desktop entries of the machine, and `open folder` opens a folder of
  the home directory. `open terminal` starts the terminal of the session.
  The folder intent starts the file manager of the session with the
  folder: `foot -e yazi "$HOME/{folder}"`, because the handler of a
  directory on this machine is the terminal program `yazi`, and a program
  that needs a terminal gets none from the daemon.
- **A window and a workspace** — `close window` closes the focused window,
  and `switch workspace`, `next workspace`, and `previous workspace` move
  between the numbered workspaces of a Hyprland session.
- **The screen and the session** — `take screenshot` writes a file into
  `~/Pictures/Screenshots`, `copy screenshot` puts a chosen area on the
  clipboard, and `lock screen` locks the session with `hyprlock`.
- **The sound** — `raise volume`, `lower volume`, and `mute sound` move
  the default output through `wpctl`.
- **What the machine knows** — `search web`, `open website`, `get
weather`, `tell time`, `show disk space`, `show memory`, and `show public
address`.
- **The desktop around the machine** — `dismiss notifications` clears the
  notification of a `mako` session, `empty clipboard` empties the
  clipboard, and `wifi off` and `wifi on` switch the wireless connection
  through NetworkManager.

A catalog of any size reaches the router, because only the short list of
one turn reaches a model. The commands are written for a Wayland session
with Hyprland,
PipeWire, `grim`/`slurp`, `wl-clipboard`, `mako`, and NetworkManager, and
they run `hyprctl` with the library path of the daemon taken off, because
the compositor links a `libstdc++` that the build shell of a daemon may
shadow; a session with other tools edits the command of the intent.

Two things the catalog leaves out on purpose. It closes a window and not
an application: an intent that stops a program by name competes with the
intent that opens one, because both act on the same value, and the phrase
`close firefox` then reads `open firefox` as well. It also leaves the
screen brightness alone: `brightnessctl` drives an internal panel, and a
desktop that is the source of this project has no panel to drive.

An entity of an intent is one of three kinds:

- **Open** takes the value the user says.
- **Closed** takes one value of the list the form holds.
- **Script** takes one value of a list a shell command answers with. The
  daemon runs the script, keeps the values in memory for a minute, and
  matches the mention against that list. `Preview` runs the script of the
  moment and lists the values it wrote, so the writer reads the list
  before a turn reads it.

A value comes from the engine or from the words of the message. The
engine reads the span of the entity, and the words read it when the engine
read nothing: a user says `firefox`, and the model reads no span for a
name the list holds, so the daemon matches the mention against the list
(`open firefox` reads `firefox`, and never the `open` of the intent
itself). A value the list spells as a path is reached through the base
name of that path as well.

The example script offers the name of the program and not the path of the
store it lives in, because a name is what the user says and what the
command of the intent runs. It reads every directory the desktop
specification names, skips the entries the desktop hides, and writes one
name per program.

The phrases of an intent are read by the resolver as well, next to the
name: a GLiNER model scores an action label low when the message names no
value for it, so a message that holds the words of a phrase names the
intent even when the model read nothing. `open firefox` holds `firefox` of
the phrase `launch firefox` and matches `open application`.

The seeded `open application` starts the program in its own session
(`setsid -f "{applications}" >/dev/null 2>&1`), so the turn answers at
once and no compositor launcher is needed. A session that launches
through its compositor writes the launcher of that compositor in the
command field instead:

- a Hyprland that reads its configuration in Lua:
  `hyprctl dispatch 'hl.dsp.exec_cmd("{applications}")'`
- a Hyprland that reads its configuration in its own language:
  `hyprctl dispatch exec "{applications}"`

A launcher prints the name of the program it started, so a turn that uses
one answers with that name. Note that the daemon passes its own library
path to the commands it runs: a shell that puts an older `libstdc++` on
that path (the CUDA shell does) makes `hyprctl` fail to load, while the
detached command above does not care.

An entity is also **required** or **optional**. The command names each
entity with a placeholder:

- A required entity the daemon read no value for stops the turn, and the
  reply asks the user for the value (`To run get weather I need a value
for city.`). The next message reaches the resolver with the asking turn
  as its history, so the user can answer with the value alone.
- An optional entity the daemon read no value for leaves no trace in the
  command: the placeholder goes, together with the whitespace it sat in
  and a quote pair that held it alone.

The **Phrases** field takes the words a user may say for the intent, one
per line. The resolver reads them next to the name of the intent.

## Transparency

The shell paints no opaque backdrop. A surface is a translucent panel, so
the window behind the page stays visible. Use the `Backdrop` control in
the status strip to switch to a solid backdrop when the page stands alone
in a browser tab.

The choice is written to the `data-backdrop` attribute of the document
element. The tokens in `src/styles/tokens.css` react to it.

```sh
npm start                                # start with the transparent backdrop
npm start -- --open '/?backdrop=solid'   # start with the solid backdrop
```

The default is transparent, so a page that is composited over a video or
a desktop window needs no setup. A standalone browser tab that shows the
text on a bright canvas should use the solid backdrop.

## Design system catalog

The route `/ui` lists every primitive with its individual states. Use it
to review a change before it reaches a page.

- The aura section holds one aura on a state at a time and moves it to
  another every five seconds, with every state also standing still below
  it. The numbers beside it are read from `src/utils/aura.ts`, which is
  where the look of a state is tuned.
- The flow section holds the `FlowGraph`, the node graph that draws blocks
  on a grid with drawn links between them. A block is a stage of a chain, a
  place a stage reads, or a turn that leaves the chain, and the graph
  reports the block a reader picks so a caller shows the settings of that
  block beside it. Pass `active` to keep one route on screen: the blocks and
  links it names hold the accent and every other one fades. The graph is a
  canvas: it fits itself to the room it is
  given, a drag moves it, and the wheel or the `−`, `+` and `Fit` controls
  change the zoom, so a graph wider than its column needs no scrollbar.
  A block lays itself out: the name stands on the first line, the state and
  the note share the wrap under it, and the block grows to hold the lines
  the note needs rather than cutting it off, so a row is as tall as its
  tallest block.
- The panel section shows the `SidePanel` as a column and as an overlay: a
  pick opens the values of the thing a reader picked beside the content on
  a wide screen and over it on a narrow one, the body scrolls on its own,
  and the footer holds the save of those values.
- A `PanelGroup` is one question of those values behind an outline of its
  own, and a `FieldLabel` names one value and keeps its sentence behind an
  `InfoHint`, so a narrow panel shows the values and not the prose.
- The tabs section holds `Tabs`, which names the parts of one context that
  do not replace one another, and the switcher section holds
  `ContentSwitcher`, which swaps between alternate views of the same
  content in one joined strip. A switch replaces what is shown, a tab
  names what else there is beside it.
- A `Card` can be one pick: `pickable` lays a press over the whole panel,
  so a list of panels is a list of choices and a row is opened by pressing
  it. The picked panel takes the accent in its border, and a control inside
  the panel keeps its own press by taking a place of its own, so a delete
  never picks the row it stands in.
- A note floats over the surface it belongs to, and the surface decides how
  much room it has. `src/utils/note-placement.ts` reads the room of the
  closest thing that clips it and places the note the way the caller asked,
  or on the other side of the mark when that side has no room, and shifts it
  along the surface until it fits, so a note is never cut off. Both the
  `InfoHint` and the `Tooltip` read that rule.
- The token section prints the computed value of every token. A typo in a
  token is visible there at once.
- The status strip carries the only navigation of the shell. It names the
  route the user is not on.

## Structure

See [[../guidelines/frontend/web/FOLDER_STRUCTURE]] for the folder rules.

| Folder                    | Content                                                       |
| ------------------------- | ------------------------------------------------------------- |
| `src/routes`              | Qwik City routes. The loaders and the default component only. |
| `src/api`                 | `server$` functions that call the backend REST API            |
| `src/schemas`             | valibot schemas                                               |
| `src/components/viz`      | The aura and the energy meter                                 |
| `src/components/sections` | One block of a page                                           |
| `src/components/ui`       | The design system primitives                                  |
| `src/components/pages`    | One complete page                                             |
| `src/context`             | The app status and the display preference                     |
| `src/lib`                 | The REST client and the socket client                         |
| `src/types`               | The DTOs of the backend contract                              |
| `src/utils`               | Pure web logic                                                |
| `src/styles`              | The design tokens                                             |
| `docs`                    | Design references, not part of the build                      |

## Design system

The look is a calm HUD with one warm accent, taken from the film _Her_.
All colors, radii, fonts, and the motion of a spinner come from
`src/styles/tokens.css`. `src/global.css` maps them to Tailwind theme
names, so a component writes `bg-ds-surface` and never a raw color.

### The aura

The aura holds the daemon state. It is a ring of light: a field that flows
around an open middle, drawn from sixteen layers, each of which warps the
face a little further along the flow and paints the ring where its warped
point crosses the radius of the ring. Where the flow is slow the layers
agree and the light is solid, and where it runs fast they part and the ring
opens into ribbons, so the light reads as moving in water rather than as a
drawn circle. How fast the flow runs, how wide the ring is, and how hard it
burns are the work of the state, so a quiet aura drifts and a working one
races.

The aura is the accent of the interface at every state of a working daemon,
and each layer is turned a little in hue from the one before it, which is
what gives the ring its depth. The one state that warns, `error`, takes the
attention tone instead, so a failure never reads as more of the same light.
The color is read out of `src/styles/tokens.css` by the component, so the
aura follows the palette of the app.

A turn that ran and a turn that failed both leave the daemon idle, so the
one state the daemon cannot report is `success`. The component watches the
count of completed turns and holds that state for a moment, then lets the
aura settle back into the state of the daemon.

It is one WebGL2 pass, `src/components/viz/aura-stage.ts`. `src/utils/aura.ts`
holds the look of every state — its tone, the pace of the flow, the width of
the ring, the shift of the hue, the breath — and `src/utils/aura-motion.ts`
carries one look into the next over about a second, so a state never cuts to
the next one.

There is no fallback renderer. A browser without WebGL2 draws nothing: a
field of sixteen warped layers per pixel cannot be rasterized on a 2D canvas
at any speed, and a still one would not be the same object.

`docs/aura-states.png` is a render of every state, in the order of
`AURA_STATES`.

What the aura costs is bounded on purpose, because the catalog on `/ui`
draws nine of them at once, one of which moves through the states: the
canvas is capped at 448 device pixels, it draws at most 30 frames a second,
and only while it is on screen. See `src/components/viz/aura.tsx`.

## API contract

The DTOs in `src/types/dto.ts` copy `backend/config/openapi.yaml`. The
chat shapes are provisional, because the daemon does not document a text
chat endpoint yet. Update the contract file when the daemon gains one.
