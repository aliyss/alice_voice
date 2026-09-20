/**
 * `ResolverSettingsSection` configures the engine that reads the intent.
 *
 * Four engines are available. The layered router reads the message in
 * stages and stops at the first stage that can answer, so every stage has
 * a reader that needs no model and a server that is down costs accuracy
 * rather than the turn. A llama.cpp server names the intent and reads the
 * entity values, and the built in GLiNER model finds the exact span of an
 * entity label. The hybrid engine uses the language model for the intent
 * and GLiNER for the values.
 *
 * Every field depends on a place. The database stores the value, the
 * llama.cpp server offers the models the user picks from, and a built in
 * model has to be on disk. A field whose place does not answer is disabled
 * and shows the reason, so the user never saves a value that cannot take
 * effect.
 *
 * A sentence the user really says can be read against the settings, so the
 * route of a message is tried before a turn depends on it: the block of
 * the message holds the sentences, and the flow lights the stages the
 * sentence that was read last really met.
 */
import type { QRL } from '@builder.io/qwik';

import type { LocalModelRole } from '~/components/sections/settings/local-models-section';
import type { FlowEdgeSpec, FlowNodeSpec } from '~/components/ui/flow';

import type {
  DecideEngine,
  DependenciesDto,
  EmbedSource,
  ExtractEngine,
  GlinerDevice,
  ListMatch,
  LocalDevice,
  ResolverPreviewDto,
  ResolverStatusDto,
  RetrieveEngine,
  SettingsDto,
} from '~/types/dto';

import type { ResolverPreviewResult } from '~/api/resolver';

import type { ResolverSettingsInput } from '~/schemas/settings';

import {
  $,
  component$,
  isBrowser,
  useSignal,
  useTask$,
} from '@builder.io/qwik';

import { GlinerFields } from '~/components/sections/settings/gliner-fields';
import { LocalModelsSection } from '~/components/sections/settings/local-models-section';
import { MessagePreviewSection } from '~/components/sections/settings/message-preview-section';
import { ServerFields } from '~/components/sections/settings/server-fields';
import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { FieldLabel } from '~/components/ui/field-label';
import { FlowGraph } from '~/components/ui/flow';
import { PanelGroup } from '~/components/ui/panel-group';
import { SidePanel } from '~/components/ui/side-panel';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

import { routeHighlight } from '~/utils/router-route';

/** The retrieval engines of the router, in the order the page shows them. */
const RETRIEVES: { value: RetrieveEngine; label: string }[] = [
  { value: 'lexical', label: 'Words' },
  { value: 'dense', label: 'Embeddings' },
  { value: 'hybrid', label: 'Both' },
];

/** The decision engines of the router, in the order the page shows them. */
const DECIDES: { value: DecideEngine; label: string }[] = [
  { value: 'score', label: 'Scores' },
  { value: 'rerank', label: 'Reranker' },
  { value: 'generative', label: 'Model' },
];

/** Where the router reads the vectors of the catalog. */
const EMBED_SOURCES: { value: EmbedSource; label: string }[] = [
  { value: 'server', label: 'Model server' },
  { value: 'local', label: 'Built in' },
];

/** The extraction engines of the router, in the order the page shows them. */
const EXTRACTS: { value: ExtractEngine; label: string }[] = [
  { value: 'lists', label: 'Lists' },
  { value: 'spans', label: 'Spans' },
  { value: 'generative', label: 'Model' },
];

/** How a mention is read against a list, in the order the page shows it. */
const LIST_MATCHES: { value: ListMatch; label: string }[] = [
  { value: 'lexical', label: 'Rules' },
  { value: 'dense', label: 'Embeddings' },
  { value: 'both', label: 'Both' },
];

/** One switch the page shows as a pair of values. */
const SWITCHES: { value: string; label: string }[] = [
  { value: 'on', label: 'On' },
  { value: 'off', label: 'Off' },
];

/** How often the section asks for the state of a running download. */
const DOWNLOAD_POLL_MS = 800;

/** One block of the router flow a reader can pick. */
type RouterBlock =
  | 'message'
  | 'deterministic'
  | 'retrieval'
  | 'decision'
  | 'extraction'
  | 'run'
  | 'builtin'
  | 'server'
  | 'gliner'
  | 'refused'
  | 'asks';

/** The name of every block of the flow. */
const BLOCK_TITLES: Record<RouterBlock, string> = {
  message: 'The message',
  deterministic: 'Deterministic pass',
  retrieval: 'Retrieval',
  decision: 'Decision',
  extraction: 'Extraction',
  run: 'The run',
  builtin: 'Built in models',
  server: 'Model server',
  gliner: 'Built in GLiNER',
  refused: 'No intent matched',
  asks: 'Asks for the value',
};

/** One sentence about every block, for the head of the settings of a block. */
const BLOCK_NOTES: Record<RouterBlock, string> = {
  message:
    'Every turn starts here with the text of one message. The turns before it reach the resolver as the history of the conversation, so the user can answer a question with a value alone.',
  deterministic:
    'Proves a message the catalog can prove: an intent the message spells out, or one value of one list that only one intent owns. It needs no model and never guesses.',
  retrieval:
    'Ranks every intent of the catalog against the message and keeps the short list. The words need no model at all; the embeddings come from the place the block beside this one names.',
  decision:
    'Chooses one of the short list. The scores read the ranking alone and need no model; the reranker reads the message and every candidate with a built in model; the language model reads only the short list, so the prompt stays small however large the catalog is.',
  extraction:
    'Reads the values of the intent that won. The lists need no model, the spans use the built in model, and the model reads every value.',
  run: 'The command of the intent runs with the values in place. A required value the router did not read stops the turn and asks the user for it instead.',
  builtin:
    'A built in model runs in the daemon through ONNX Runtime, so it needs no server and no network after the download. The device is shared by every built in model.',
  server:
    'The llama.cpp server answers the embedding requests of a stage and the chat requests of a generative stage. The address is one database setting, so every stage that reads it writes the same value.',
  gliner:
    'The built in GLiNER model finds the exact span of every label of the intent that won. A label carries no value when the model reads nothing, which leaves the value to the language model.',
  refused:
    'The turn ends with no intent matched. Nothing runs, and the reply names the reader that refused.',
  asks: 'The turn stops before the command and asks the user for the value the entity needs. The next message reaches the resolver with the asking turn as its history.',
};

/** The props of `ChoiceRow`. */
interface ChoiceRowProps {
  /** The name of the setting. */
  label: string;
  /** One sentence about what the choice changes. */
  note: string;
  /** The value the row shows as chosen. */
  value: string;
  /** The values the row offers. */
  options: { value: string; label: string }[];
  /** True while a save runs or the value cannot be stored. */
  disabled: boolean;
  /** Report the value the user chose. */
  onPick$: QRL<(next: string) => void>;
}

/**
 * One setting of the router, shown as the values it can take.
 *
 * Every stage of the router is a small choice among readers, so the page
 * shows the readers rather than a free text field that could hold a value
 * the daemon does not know. The sentence about the choice waits in the
 * hint of its name, so the panel shows the values and not the prose.
 */
const ChoiceRow = component$<ChoiceRowProps>((props) => (
  <Stack gap="xs">
    <FieldLabel label={props.label} hint={props.note} />
    <ContentSwitcher
      ariaLabel={props.label}
      value={props.value}
      options={props.options}
      disabled={props.disabled}
      onPick$={props.onPick$}
    />
  </Stack>
));

/** The props of `NumberRow`. */
interface NumberRowProps {
  /** The name of the setting. */
  label: string;
  /** One sentence about what the number changes. */
  note: string;
  /** The name the field reports to the browser. */
  name: string;
  /** The value the field shows. */
  value: string;
  /** The value the field shows while it is empty. */
  placeholder: string;
  /** True while a save runs or the value cannot be stored. */
  disabled: boolean;
  /** Report the value the user typed. */
  onInput$: QRL<(next: string) => void>;
}

/** One number of the router, with the sentence that explains it. */
const NumberRow = component$<NumberRowProps>((props) => (
  <Stack gap="xs">
    <FieldLabel label={props.label} hint={props.note} />
    <TextInput
      kind="input"
      surface="field"
      name={props.name}
      ariaLabel={props.label}
      placeholder={props.placeholder}
      value={props.value}
      disabled={props.disabled}
      onInput$={$((event: Event) => {
        props.onInput$((event.target as HTMLInputElement).value);
      })}
    />
  </Stack>
));

/** The props of `ResolverSettingsSection`. */
export interface ResolverSettingsSectionProps {
  /** The stored settings of the daemon. */
  settings: SettingsDto;
  /** The state of the resolver, or null while it loads. */
  status: ResolverStatusDto | null;
  /** The state of the places the settings depend on, or null. */
  dependencies: DependenciesDto | null;
  /** True while a save request is in flight. */
  pending: boolean;
  /** True while a download or a remove request is in flight. */
  installing: boolean;
  /** The last failure message, or null. */
  error: string | null;
  /** True after a save that the daemon stored. */
  saved: boolean;
  /** Save the values. The page defines the handle. */
  onSave$: QRL<(input: ResolverSettingsInput) => void>;
  /** Download one built in GLiNER model. */
  onDownloadModel$: QRL<(id: string) => void>;
  /** Remove the files of one built in GLiNER model. */
  onRemoveModel$: QRL<(id: string) => void>;
  /** Download one built in model of the router. */
  onDownloadLocalModel$: QRL<(id: string) => void>;
  /** Remove the files of one built in model of the router. */
  onRemoveLocalModel$: QRL<(id: string) => void>;
  /** Read the state of the resolver again. */
  onRefreshStatus$: QRL<() => void>;
  /** Report that a value changed, so the stored values are stale. */
  onEdit$: QRL<() => void>;
  /**
   * Read one sentence without running anything, so the user sees the route
   * of a message before a turn depends on it.
   */
  onPreviewMessage$: QRL<(text: string) => Promise<ResolverPreviewResult>>;
  /**
   * Store the sentences the user tries against the resolver, so the tests
   * of a pipeline outlive the page that wrote them.
   */
  onStoreSentences$: QRL<(sentences: string[]) => void>;
}

export const ResolverSettingsSection = component$<ResolverSettingsSectionProps>(
  (props) => {
    const baseUrl = useSignal(props.settings.resolverBaseUrl);
    const model = useSignal(props.settings.resolverModel);
    const glinerModel = useSignal(props.settings.glinerModel);
    const glinerDevice = useSignal(props.settings.glinerDevice);
    const threshold = useSignal(String(props.settings.glinerThreshold));
    const fastPath = useSignal(props.settings.routerFastPath);
    const retrieve = useSignal(props.settings.routerRetrieve);
    const decide = useSignal(props.settings.routerDecide);
    const extract = useSignal(props.settings.routerExtract);
    const topK = useSignal(String(props.settings.routerTopK));
    const floor = useSignal(String(props.settings.routerFloor));
    const margin = useSignal(String(props.settings.routerMargin));
    const lexicalWeight = useSignal(String(props.settings.routerLexicalWeight));
    const denseWeight = useSignal(String(props.settings.routerDenseWeight));
    const embedModel = useSignal(props.settings.routerEmbedModel);
    const embedSource = useSignal(props.settings.routerEmbedSource);
    const embedLocalModel = useSignal(props.settings.routerEmbedLocalModel);
    const rerankModel = useSignal(props.settings.routerRerankModel);
    const localDevice = useSignal(props.settings.routerLocalDevice);
    const phraseGate = useSignal(props.settings.routerPhraseGate);
    const listMatch = useSignal(props.settings.routerListMatch);
    const listFloor = useSignal(String(props.settings.routerListFloor));
    const selectedBlock = useSignal<RouterBlock>('retrieval');
    // The tests of the pipeline. A sentence is stored as soon as the user
    // adds or removes one, so the list the page shows is the list the
    // daemon holds.
    const sentences = useSignal<string[]>(props.settings.previewSentences);
    // The route of the sentence that was read last. The flow lights the
    // stages it met, so a user tunes the pipeline against a message they
    // really send.
    const preview = useSignal<ResolverPreviewDto | null>(null);
    // The values of a step live in the panel beside the flow. A step that is
    // picked opens the panel, and a reader on a narrow screen closes it to
    // go back to the flow.
    const panelOpen = useSignal(true);

    const dependencies = props.dependencies;
    const canStore = dependencies?.database.reachable === true;
    const llamaReachable = dependencies?.llama.reachable === true;
    const llamaModels = dependencies?.llama.models ?? [];
    // The router draws the server and the built in model inside the stage
    // that reads them, so the two shared blocks below belong to the other
    // engines.
    const gliner = props.status?.gliner ?? null;
    const chosen = gliner?.models.find(
      (entry) => entry.id === glinerModel.value,
    );
    const chosenName = chosen?.name ?? glinerModel.value;
    const router = props.status?.router ?? null;
    const localStore = router?.local ?? null;

    // The stages the current settings read decide which built in models
    // the section offers, so a router that reads the words alone and
    // decides by the scores needs no file at all.
    const readsVectors =
      retrieve.value !== 'lexical' || listMatch.value !== 'lexical';
    // The built in model belongs to the stage that reads it, so the flow
    // offers the embedding model on the retrieval stage and the reranker on
    // the decision stage rather than in one block of its own.
    const embedRoles: LocalModelRole[] =
      embedSource.value === 'local' && readsVectors ? ['embeddings'] : [];
    const rerankRoles: LocalModelRole[] =
      decide.value === 'rerank' ? ['reranker'] : [];
    const localRoles: LocalModelRole[] = [...embedRoles, ...rerankRoles];
    const localChosen: Record<LocalModelRole, string> = {
      embeddings: embedLocalModel.value,
      reranker: rerankModel.value,
    };

    // The reader every stage would run right now, so the flow names the
    // reader on the stage that runs it rather than in a list of its own.
    const retrieveLabel =
      RETRIEVES.find((entry) => entry.value === retrieve.value)?.label ??
      'Words';
    const decideLabel =
      DECIDES.find((entry) => entry.value === decide.value)?.label ?? 'Scores';
    const extractLabel =
      EXTRACTS.find((entry) => entry.value === extract.value)?.label ?? 'Lists';
    const shortList = topK.value.trim() || '8';

    // The places the current stages really read. A place is drawn on the
    // graph only when a stage reads it, so the graph never shows a server
    // or a model file a turn would not touch.
    const usesServer =
      (embedSource.value === 'server' && readsVectors) ||
      decide.value === 'generative' ||
      extract.value === 'generative';
    const usesBuiltIn =
      (embedSource.value === 'local' && readsVectors) ||
      decide.value === 'rerank';
    const readsGliner = extract.value === 'spans';

    // The graph reads left to right: the places a step reads stand in the
    // first column, the chain of the message stands in the middle, and the
    // turns that leave the chain stand in the last column. Every link
    // therefore runs forward, so a reader never has to walk a link
    // backwards to see where it goes.
    const flowNodes: FlowNodeSpec[] = [
      {
        id: 'message',
        title: 'The message',
        detail: 'one text, with the turns before it',
        kind: 'stage',
        column: 1,
        row: 0,
        stage: 1,
      },
      {
        id: 'deterministic',
        title: 'Deterministic pass',
        detail: 'proves what the catalog can prove',
        status: { label: fastPath.value ? 'On' : 'Off' },
        kind: 'stage',
        column: 1,
        row: 1,
        stage: 2,
      },
      {
        id: 'retrieval',
        title: 'Retrieval',
        detail: 'ranks the catalog, keeps the short list',
        status: {
          label: retrieveLabel,
          tone:
            retrieve.value !== 'lexical' && !router?.embeddingsReachable
              ? 'warn'
              : 'neutral',
        },
        kind: 'stage',
        column: 1,
        row: 2,
        stage: 3,
      },
      {
        id: 'decision',
        title: 'Decision',
        detail: 'chooses one of the short list, or none',
        status: { label: decideLabel },
        kind: 'stage',
        column: 1,
        row: 3,
        stage: 4,
      },
      {
        id: 'extraction',
        title: 'Extraction',
        detail: 'reads the values of the intent that won',
        status: { label: extractLabel },
        kind: 'stage',
        column: 1,
        row: 4,
        stage: 5,
      },
      {
        id: 'run',
        title: 'The run',
        detail: 'runs the command with the values in place',
        status: { label: 'Command' },
        kind: 'stage',
        column: 1,
        row: 5,
        stage: 6,
      },
      {
        id: 'refused',
        title: 'No intent matched',
        detail: 'nothing runs, the reply names the reader',
        kind: 'exit',
        // An exit sits on the row of the stage it leaves, so its link runs
        // straight out of that stage. Stacking the exits below the chain
        // instead would make the two links cross, because the decision
        // leaves above the extraction and refuses below it.
        column: 2,
        row: 3,
      },
      {
        id: 'asks',
        title: 'Asks for the value',
        detail: 'a required value is missing',
        kind: 'exit',
        column: 2,
        row: 4,
      },
    ];

    if (usesBuiltIn) {
      flowNodes.push({
        id: 'builtin',
        title: 'Built in models',
        detail:
          decide.value === 'rerank' ? rerankModel.value : embedLocalModel.value,
        status: {
          label: localDevice.value === 'auto' ? 'Auto' : localDevice.value,
        },
        kind: 'place',
        column: 0,
        row: 2,
      });
    }

    if (usesServer) {
      flowNodes.push({
        id: 'server',
        title: 'Model server',
        detail: `${model.value} at ${baseUrl.value}`,
        status: {
          label: llamaReachable ? 'Up' : 'Down',
          tone: llamaReachable ? 'ok' : 'error',
        },
        kind: 'place',
        column: 0,
        row: 3,
      });
    }

    if (readsGliner) {
      flowNodes.push({
        id: 'gliner',
        title: 'Built in GLiNER',
        detail: chosenName,
        status: {
          label: gliner?.installed ? 'On disk' : 'Missing',
          tone: gliner?.installed ? 'ok' : 'warn',
        },
        kind: 'place',
        column: 0,
        row: 4,
      });
    }

    // The route of the sentence that was read last, read back onto the
    // blocks of this graph. A place a stage read lights up only when the
    // reader that really answered it stands in the graph.
    const highlight = routeHighlight(preview.value, {
      vectors:
        retrieve.value !== 'lexical' && readsVectors
          ? embedSource.value === 'local'
            ? 'builtin'
            : 'server'
          : null,
      reranker: decide.value === 'rerank' ? 'builtin' : null,
      model: usesServer ? 'server' : null,
      spans: readsGliner ? 'gliner' : null,
    });

    const flowEdges: FlowEdgeSpec[] = [
      {
        id: 'message-deterministic',
        from: 'message',
        to: 'deterministic',
        label: 'one message',
        kind: 'main',
      },
      {
        id: 'deterministic-retrieval',
        from: 'deterministic',
        to: 'retrieval',
        label: 'not proven',
        kind: 'main',
      },
      {
        id: 'retrieval-decision',
        from: 'retrieval',
        to: 'decision',
        label: `short list of ${shortList}`,
        kind: 'main',
      },
      {
        id: 'decision-extraction',
        from: 'decision',
        to: 'extraction',
        label: 'one intent',
        kind: 'main',
      },
      {
        id: 'extraction-run',
        from: 'extraction',
        to: 'run',
        label: 'the values',
        kind: 'main',
      },
      {
        id: 'decision-refused',
        from: 'decision',
        to: 'refused',
        label: 'none',
        kind: 'branch',
      },
      {
        id: 'extraction-asks',
        from: 'extraction',
        to: 'asks',
        label: 'value missing',
        kind: 'branch',
      },
    ];

    if (usesBuiltIn) {
      if (embedSource.value === 'local' && readsVectors) {
        flowEdges.push({
          id: 'builtin-retrieval',
          from: 'builtin',
          to: 'retrieval',
          label: 'embeddings',
          kind: 'branch',
        });
      }
      if (decide.value === 'rerank') {
        flowEdges.push({
          id: 'builtin-decision',
          from: 'builtin',
          to: 'decision',
          label: 'reranker',
          kind: 'branch',
        });
      }
    }

    if (usesServer) {
      if (embedSource.value === 'server' && readsVectors) {
        flowEdges.push({
          id: 'server-retrieval',
          from: 'server',
          to: 'retrieval',
          label: 'embeddings',
          kind: 'branch',
        });
      }
      if (decide.value === 'generative') {
        flowEdges.push({
          id: 'server-decision',
          from: 'server',
          to: 'decision',
          label: 'chat',
          kind: 'branch',
        });
      }
      if (extract.value === 'generative') {
        flowEdges.push({
          id: 'server-extraction',
          from: 'server',
          to: 'extraction',
          label: 'chat',
          kind: 'branch',
        });
      }
    }

    if (readsGliner) {
      flowEdges.push({
        id: 'gliner-extraction',
        from: 'gliner',
        to: 'extraction',
        label: 'spans',
        kind: 'branch',
      });
    }

    const handleSelectBlock = $((id: string) => {
      selectedBlock.value = id as RouterBlock;
      panelOpen.value = true;
    });

    const handleClosePanel = $(() => {
      panelOpen.value = false;
    });

    const handleChooseLocalModel = $((role: LocalModelRole, id: string) => {
      if (role === 'embeddings') {
        embedLocalModel.value = id;
      } else {
        rerankModel.value = id;
      }
      props.onEdit$();
    });

    const handleLocalDevice = $((next: LocalDevice) => {
      localDevice.value = next;
      props.onEdit$();
    });

    /** Why the settings cannot be stored, or null. */
    const storeReason = canStore
      ? null
      : dependencies
        ? `The ${dependencies.database.label.toLowerCase()} does not answer, so the daemon cannot store the value.${
            dependencies.database.detail
              ? ` ${dependencies.database.detail}`
              : ''
          }`
        : 'The daemon does not answer, so the daemon cannot store the value.';

    /** The models the dropdown offers, with the stored one when the server has none of it. */
    const modelOptions = (() => {
      const ids = [...llamaModels];
      if (model.value.length > 0 && !ids.includes(model.value)) {
        ids.unshift(model.value);
      }
      return ids.map((id) => ({
        value: id,
        label:
          id === model.value && !llamaModels.includes(id)
            ? `${id} (stored, not offered)`
            : id,
      }));
    })();

    // A download of a large model takes minutes, so the section reads the
    // state of the resolver while one runs. The daemon reports the bytes
    // it wrote, so the progress is a real number and not an estimate. A
    // download of a built in model of the router is polled the same way.
    useTask$(({ track, cleanup }) => {
      const running = track(() => {
        const state =
          props.status?.gliner.download ?? props.status?.router.local.download;
        return state && !state.done && !state.error ? state.model : null;
      });
      if (!isBrowser || !running) {
        return;
      }
      const timer = window.setInterval(() => {
        void props.onRefreshStatus$();
      }, DOWNLOAD_POLL_MS);
      cleanup(() => window.clearInterval(timer));
    });

    const handleBaseUrl = $((value: string) => {
      baseUrl.value = value;
      props.onEdit$();
    });

    const handleModel = $((value: string) => {
      model.value = value;
      props.onEdit$();
    });

    const handleThreshold = $((value: string) => {
      threshold.value = value;
      props.onEdit$();
    });

    const handleDevice = $((next: GlinerDevice) => {
      glinerDevice.value = next;
      props.onEdit$();
    });

    const handleChooseModel = $((id: string) => {
      glinerModel.value = id;
      props.onEdit$();
    });

    const handleSave = $(() => {
      props.onSave$({
        // The layered router is the engine of every turn now, so the form
        // writes it rather than offering a choice that changes the pipeline.
        resolverBackend: 'router' as const,
        resolverBaseUrl: baseUrl.value,
        resolverModel: model.value,
        glinerModel: glinerModel.value,
        glinerDevice: glinerDevice.value,
        glinerThreshold: Number(threshold.value),
        routerFastPath: fastPath.value,
        routerRetrieve: retrieve.value,
        routerDecide: decide.value,
        routerExtract: extract.value,
        routerTopK: Number(topK.value),
        routerFloor: Number(floor.value),
        routerMargin: Number(margin.value),
        routerLexicalWeight: Number(lexicalWeight.value),
        routerDenseWeight: Number(denseWeight.value),
        routerEmbedModel: embedModel.value,
        routerModelsDir: props.settings.routerModelsDir,
        routerEmbedSource: embedSource.value,
        routerEmbedLocalModel: embedLocalModel.value,
        routerRerankModel: rerankModel.value,
        routerLocalDevice: localDevice.value,
        routerPhraseGate: phraseGate.value,
        routerListMatch: listMatch.value,
        routerListFloor: Number(listFloor.value),
      });
    });

    // The flow fills the room the section leaves and the values of the
    // picked step take a panel of their own beside it, so a pick shifts the
    // card rather than nesting a second surface in it, and the page never
    // scrolls a pipeline that is taller than the screen.
    return (
      <Box class="flex h-full min-h-0 flex-col gap-6 lg:flex-row">
        <Card
          class="flex h-full min-h-0 w-full min-w-0 flex-1 flex-col"
          bodyClass="flex min-h-0 flex-1 flex-col"
        >
          <Stack gap="md" class="min-h-0 flex-1">
            {storeReason ? (
              <Alert
                tone="error"
                title="The settings cannot be stored"
                message={storeReason}
              />
            ) : null}

            {/**
             * The flow fills the room the card leaves, so the whole pipeline
             * stays in view without the page scrolling it. The values of the
             * picked step take the panel beside the card, so a step and its
             * values are one view and a pick shifts the flow rather than
             * adding a block below it. The flow is a canvas that fits itself
             * to the card and pans and zooms, so a wide graph needs no
             * scrollbar of its own.
             */}
            <Stack gap="md" class="min-h-0 flex-1">
              <Stack gap="sm">
                <Stack
                  direction="row"
                  gap="sm"
                  align="center"
                  justify="between"
                >
                  <Text size="body" weight="medium">
                    Layered router
                  </Text>
                  <Badge
                    tone={router?.embeddingsReachable ? 'ok' : 'neutral'}
                    label={
                      router?.embeddingsReachable
                        ? 'Embeddings up'
                        : 'Words only'
                    }
                  />
                </Stack>
                <Text size="micro" tone="faint">
                  The message passes through the stages below and stops at the
                  first stage that can answer. Every stage is chosen in the flow
                  itself, and every stage has a reader that needs no model, so a
                  server that is down costs accuracy rather than the turn.
                </Text>
                {router && !router.embeddingsReachable ? (
                  <Text size="micro" tone="faint">
                    {router.embeddingsDetail ??
                      'The embedding server does not answer, so the words of the catalog rank it alone.'}
                  </Text>
                ) : null}
              </Stack>

              <FlowGraph
                id="router-flow"
                ariaLabel="The stages of the layered router"
                selected={selectedBlock.value}
                nodes={flowNodes}
                edges={flowEdges}
                route={highlight?.route ?? null}
                fill
                onSelect$={handleSelectBlock}
              />

              {highlight ? (
                <Stack
                  direction="row"
                  gap="sm"
                  align="center"
                  justify="between"
                >
                  <Text size="hud" tone="faint">
                    The flow shows the route of the sentence that was read last.
                  </Text>
                  <Button
                    variant="quiet"
                    size="sm"
                    onClick$={$(() => {
                      preview.value = null;
                    })}
                  >
                    Show every stage
                  </Button>
                </Stack>
              ) : null}
            </Stack>
          </Stack>
        </Card>

        <SidePanel
          footer
          ariaLabel="The values of one step of the layered router"
          title={BLOCK_TITLES[selectedBlock.value]}
          note={BLOCK_NOTES[selectedBlock.value]}
          open={panelOpen.value}
          onClose$={handleClosePanel}
        >
          <Stack gap="md">
            {selectedBlock.value === 'message' ? (
              <MessagePreviewSection
                sentences={sentences.value}
                disabled={props.pending || !canStore}
                onPreview$={props.onPreviewMessage$}
                onSentences$={$((next: string[]) => {
                  sentences.value = next;
                  props.onStoreSentences$(next);
                })}
                onRoute$={$((next: ResolverPreviewDto | null) => {
                  preview.value = next;
                })}
              />
            ) : null}

            {selectedBlock.value === 'deterministic' ? (
              <>
                <ChoiceRow
                  label="Runs first"
                  note="Off reads every message in the stages below."
                  value={fastPath.value ? 'on' : 'off'}
                  options={SWITCHES}
                  disabled={props.pending || !canStore}
                  onPick$={$((next: string) => {
                    fastPath.value = next === 'on';
                    props.onEdit$();
                  })}
                />
              </>
            ) : null}

            {selectedBlock.value === 'retrieval' ? (
              <>
                <ChoiceRow
                  label="Reader"
                  note="The words of the catalog, the embeddings, or both folded into one score."
                  value={retrieve.value}
                  options={RETRIEVES}
                  disabled={props.pending || !canStore}
                  onPick$={$((next: string) => {
                    retrieve.value = next as RetrieveEngine;
                    props.onEdit$();
                  })}
                />

                <PanelGroup
                  title="Folding the two"
                  hint="The two weights fold the words and the embeddings into one score. A weight of zero drops that evidence."
                >
                  <Stack direction="row" gap="sm" wrap>
                    <Stack gap="xs" class="min-w-40 flex-1">
                      <FieldLabel
                        label="Weight of the words"
                        hint="How much the words of the catalog count. Zero reads the embeddings alone."
                      />
                      <TextInput
                        kind="input"
                        surface="field"
                        name="routerLexicalWeight"
                        ariaLabel="Weight of the words"
                        placeholder="1"
                        value={lexicalWeight.value}
                        disabled={props.pending || !canStore}
                        onInput$={$((event: Event) => {
                          lexicalWeight.value = (
                            event.target as HTMLInputElement
                          ).value;
                          props.onEdit$();
                        })}
                      />
                    </Stack>
                    <Stack gap="xs" class="min-w-40 flex-1">
                      <FieldLabel
                        label="Weight of the embeddings"
                        hint="How much the embeddings of the catalog count. Zero reads the words alone."
                      />
                      <TextInput
                        kind="input"
                        surface="field"
                        name="routerDenseWeight"
                        ariaLabel="Weight of the embeddings"
                        placeholder="1"
                        value={denseWeight.value}
                        disabled={props.pending || !canStore}
                        onInput$={$((event: Event) => {
                          denseWeight.value = (
                            event.target as HTMLInputElement
                          ).value;
                          props.onEdit$();
                        })}
                      />
                    </Stack>
                  </Stack>

                  <ChoiceRow
                    label="Phrase gate"
                    note="A phrase the user wrote counts only when the message shares its action. Without the gate the phrase `close firefox` reads `open firefox` as a close."
                    value={phraseGate.value ? 'on' : 'off'}
                    options={SWITCHES}
                    disabled={props.pending || !canStore}
                    onPick$={$((next: string) => {
                      phraseGate.value = next === 'on';
                      props.onEdit$();
                    })}
                  />
                </PanelGroup>

                <PanelGroup
                  title="Where the vectors come from"
                  hint="The model server answers the `/embeddings` path of its address and needs no download. A built in model runs in the daemon and needs no server and no network after the download."
                >
                  <ChoiceRow
                    label="Embeddings from"
                    note="The words of the catalog need no model at all, so this choice only counts while the reader above reads embeddings."
                    value={embedSource.value}
                    options={EMBED_SOURCES}
                    disabled={props.pending || !canStore}
                    onPick$={$((next: string) => {
                      embedSource.value = next as EmbedSource;
                      props.onEdit$();
                    })}
                  />

                  {embedSource.value === 'server' ? (
                    <Stack gap="md">
                      <ServerFields
                        baseUrl={baseUrl.value}
                        model={model.value}
                        modelOptions={modelOptions}
                        resolvedBaseUrl={dependencies?.llama.baseUrl ?? null}
                        canStore={canStore}
                        llamaReachable={llamaReachable}
                        pending={props.pending}
                        showModel={false}
                        onBaseUrl$={handleBaseUrl}
                        onModel$={handleModel}
                        budget={props.status?.budget ?? null}
                      />
                      <Stack gap="xs">
                        <FieldLabel
                          label="Embedding model"
                          hint={`The server offers its embedding models at ${
                            dependencies?.llama.baseUrl ?? 'the address above'
                          }/models, next to the chat models it offers.`}
                        />
                        <TextInput
                          kind="input"
                          surface="field"
                          name="routerEmbedModel"
                          ariaLabel="Model name of the embedding server"
                          placeholder="bge-small-en-v1.5"
                          value={embedModel.value}
                          disabled={props.pending || !canStore}
                          onInput$={$((event: Event) => {
                            embedModel.value = (
                              event.target as HTMLInputElement
                            ).value;
                            props.onEdit$();
                          })}
                        />
                      </Stack>
                    </Stack>
                  ) : null}

                  {embedRoles.length > 0 ? (
                    <LocalModelsSection
                      store={localStore}
                      roles={embedRoles}
                      chosen={localChosen}
                      device={localDevice.value}
                      devices={gliner?.devices ?? []}
                      cudaBuild={gliner?.cudaBuild ?? false}
                      pending={props.pending}
                      installing={props.installing}
                      onChoose$={handleChooseLocalModel}
                      onDevice$={handleLocalDevice}
                      onDownload$={props.onDownloadLocalModel$}
                      onRemove$={props.onRemoveLocalModel$}
                    />
                  ) : null}

                  {localRoles.length === 0 ? (
                    <Text size="hud" tone="faint">
                      No stage of the router reads a built in model right now,
                      so no file is needed here.
                    </Text>
                  ) : null}
                </PanelGroup>
              </>
            ) : null}

            {selectedBlock.value === 'decision' ? (
              <>
                <ChoiceRow
                  label="Reader"
                  note="How the stage chooses among the short list."
                  value={decide.value}
                  options={DECIDES}
                  disabled={props.pending || !canStore}
                  onPick$={$((next: string) => {
                    decide.value = next as DecideEngine;
                    props.onEdit$();
                  })}
                />

                {decide.value === 'generative' ? (
                  <ServerFields
                    baseUrl={baseUrl.value}
                    model={model.value}
                    modelOptions={modelOptions}
                    resolvedBaseUrl={dependencies?.llama.baseUrl ?? null}
                    canStore={canStore}
                    llamaReachable={llamaReachable}
                    pending={props.pending}
                    onBaseUrl$={handleBaseUrl}
                    onModel$={handleModel}
                    budget={props.status?.budget ?? null}
                  />
                ) : null}

                <NumberRow
                  label="Short list"
                  name="routerTopK"
                  placeholder="8"
                  value={topK.value}
                  disabled={props.pending || !canStore}
                  note="How many intents the decision stage reads. A larger list reads more surely and costs a longer prompt."
                  onInput$={$((next: string) => {
                    topK.value = next;
                    props.onEdit$();
                  })}
                />

                <NumberRow
                  label="Smallest score the decision accepts"
                  name="routerFloor"
                  placeholder="0.4"
                  value={floor.value}
                  disabled={props.pending || !canStore}
                  note="Below this the turn refuses instead of running a command, so a message the catalog does not hold stays a refusal."
                  onInput$={$((next: string) => {
                    floor.value = next;
                    props.onEdit$();
                  })}
                />

                <NumberRow
                  label="Smallest distance between the best two"
                  name="routerMargin"
                  placeholder="0.1"
                  value={margin.value}
                  disabled={props.pending || !canStore}
                  note="When two intents fit a message equally well the turn refuses, because running the wrong command is worse than refusing."
                  onInput$={$((next: string) => {
                    margin.value = next;
                    props.onEdit$();
                  })}
                />

                {rerankRoles.length > 0 ? (
                  <LocalModelsSection
                    store={localStore}
                    roles={rerankRoles}
                    chosen={localChosen}
                    device={localDevice.value}
                    devices={gliner?.devices ?? []}
                    cudaBuild={gliner?.cudaBuild ?? false}
                    showDevice={embedRoles.length === 0}
                    pending={props.pending}
                    installing={props.installing}
                    onChoose$={handleChooseLocalModel}
                    onDevice$={handleLocalDevice}
                    onDownload$={props.onDownloadLocalModel$}
                    onRemove$={props.onRemoveLocalModel$}
                  />
                ) : null}
              </>
            ) : null}

            {selectedBlock.value === 'extraction' ? (
              <>
                <ChoiceRow
                  label="Reader"
                  note="How the stage reads the values of the intent."
                  value={extract.value}
                  options={EXTRACTS}
                  disabled={props.pending || !canStore}
                  onPick$={$((next: string) => {
                    extract.value = next as ExtractEngine;
                    props.onEdit$();
                  })}
                />

                {extract.value === 'generative' ? (
                  <ServerFields
                    baseUrl={baseUrl.value}
                    model={model.value}
                    modelOptions={modelOptions}
                    resolvedBaseUrl={dependencies?.llama.baseUrl ?? null}
                    canStore={canStore}
                    llamaReachable={llamaReachable}
                    pending={props.pending}
                    onBaseUrl$={handleBaseUrl}
                    onModel$={handleModel}
                    budget={props.status?.budget ?? null}
                  />
                ) : null}

                {extract.value === 'spans' && gliner ? (
                  <GlinerFields
                    gliner={gliner}
                    model={glinerModel.value}
                    chosenName={chosenName}
                    device={glinerDevice.value}
                    threshold={threshold.value}
                    canStore={canStore}
                    pending={props.pending}
                    installing={props.installing}
                    backend="router"
                    onChoose$={handleChooseModel}
                    onDevice$={handleDevice}
                    onThreshold$={handleThreshold}
                    onDownload$={props.onDownloadModel$}
                    onRemove$={props.onRemoveModel$}
                    budget={props.status?.budget ?? null}
                  />
                ) : null}

                <ChoiceRow
                  label="Reading a value of a list"
                  note="The rules of the matcher read the spelling; the embeddings read the meaning, so `the mail client` reaches `thunderbird`."
                  value={listMatch.value}
                  options={LIST_MATCHES}
                  disabled={props.pending || !canStore}
                  onPick$={$((next: string) => {
                    listMatch.value = next as ListMatch;
                    props.onEdit$();
                  })}
                />

                <NumberRow
                  label="Smallest similarity a value needs"
                  name="routerListFloor"
                  placeholder="0.8"
                  value={listFloor.value}
                  disabled={props.pending || !canStore}
                  note="An embedding match below this leaves the entity without a value, so a list of unrelated names stays quiet."
                  onInput$={$((next: string) => {
                    listFloor.value = next;
                    props.onEdit$();
                  })}
                />
              </>
            ) : null}

            {selectedBlock.value === 'builtin' ? (
              <>
                {embedRoles.length + rerankRoles.length > 0 ? (
                  <LocalModelsSection
                    store={localStore}
                    roles={[...embedRoles, ...rerankRoles]}
                    chosen={localChosen}
                    device={localDevice.value}
                    devices={gliner?.devices ?? []}
                    cudaBuild={gliner?.cudaBuild ?? false}
                    pending={props.pending}
                    installing={props.installing}
                    onChoose$={handleChooseLocalModel}
                    onDevice$={handleLocalDevice}
                    onDownload$={props.onDownloadLocalModel$}
                    onRemove$={props.onRemoveLocalModel$}
                  />
                ) : (
                  <Text size="micro" tone="faint">
                    No stage of the router reads a built in model right now, so
                    no file is needed. Point a stage at the built in reader to
                    configure one here.
                  </Text>
                )}
              </>
            ) : null}

            {selectedBlock.value === 'server' ? (
              <>
                <ServerFields
                  baseUrl={baseUrl.value}
                  model={model.value}
                  modelOptions={modelOptions}
                  resolvedBaseUrl={dependencies?.llama.baseUrl ?? null}
                  canStore={canStore}
                  llamaReachable={llamaReachable}
                  pending={props.pending}
                  onBaseUrl$={handleBaseUrl}
                  onModel$={handleModel}
                  budget={props.status?.budget ?? null}
                />
                <Text size="micro" tone="faint">
                  A stage that reads embeddings points this address at the path
                  of the embedding endpoint and names its own embedding model on
                  the retrieval stage. A generative stage sends its chat request
                  to the model above.
                </Text>
              </>
            ) : null}

            {selectedBlock.value === 'gliner' && gliner ? (
              <>
                <GlinerFields
                  gliner={gliner}
                  model={glinerModel.value}
                  chosenName={chosenName}
                  device={glinerDevice.value}
                  threshold={threshold.value}
                  canStore={canStore}
                  pending={props.pending}
                  installing={props.installing}
                  backend="router"
                  onChoose$={handleChooseModel}
                  onDevice$={handleDevice}
                  onThreshold$={handleThreshold}
                  onDownload$={props.onDownloadModel$}
                  onRemove$={props.onRemoveModel$}
                  budget={props.status?.budget ?? null}
                />
              </>
            ) : null}
          </Stack>

          {/**
           * The save of the resolver belongs to the values of the flow, so
           * it takes the footer of the panel: a reader who changed a stage
           * at the top of a long list reaches it without scrolling. The
           * answer of the last save stands with it.
           */}
          <Box q:slot="footer">
            <Stack gap="sm">
              {props.error ? (
                <Alert tone="error" title="Save failed" message={props.error} />
              ) : null}

              {props.saved && !props.error ? (
                <Alert
                  tone="info"
                  title="Saved"
                  message="The daemon uses these values for the next message."
                />
              ) : null}

              <Stack direction="row" gap="sm" align="center" justify="end">
                <Button
                  variant="solid"
                  size="md"
                  disabled={props.pending || !canStore}
                  onClick$={handleSave}
                >
                  {props.pending ? 'Saving...' : 'Save resolver'}
                </Button>
              </Stack>
            </Stack>
          </Box>
        </SidePanel>
      </Box>
    );
  },
);
