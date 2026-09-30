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
  LibrarianStatusDto,
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
import { SystemSlidersSection } from '~/components/sections/settings/system-sliders-section';
import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { FieldLabel } from '~/components/ui/field-label';
import { FlowGraph } from '~/components/ui/flow';
import { MetadataRow } from '~/components/ui/metadata-row';
import { PanelGroup } from '~/components/ui/panel-group';
import { SidePanel } from '~/components/ui/side-panel';
import { Stack } from '~/components/ui/stack';
import { Tabs } from '~/components/ui/tabs';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';
import { ToggleSwitch } from '~/components/ui/toggle-switch';

import { isCustomForQuality, presetForQuality } from '~/utils/preset';
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
  { value: 'laya', label: 'Laya' },
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
  { value: 'laya', label: 'Laya' },
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

/**
 * One place a stage reads.
 *
 * A stage owns what it does and the place it reads owns how it connects,
 * so the stage names the place and the block of that place holds the
 * address, the file, and the device. A stage therefore shows one row per
 * place it reads, and the row opens the block rather than repeating the
 * connection inside the stage.
 */
export interface PlaceRead {
  /** What the stage reads there. */
  detail: string;
  /** The name of the place, as the block shows it. */
  place: string;
  /** The tone of the state of the place. */
  tone: 'ok' | 'warn' | 'neutral' | 'error';
  /** The id of the block that holds the values of the place. */
  block: RouterBlock;
}

/** The props of `ReaderRow`. */
export interface ReaderRowProps {
  /** What the stage reads there. */
  detail: string;
  /** The name of the place, as the block shows it. */
  place: string;
  /** The tone of the state of the place. */
  tone?: 'ok' | 'warn' | 'neutral' | 'error';
  /** The id of the block that holds the values of the place. */
  block: RouterBlock;
  /** Open the block of the place. */
  onOpen$: QRL<(id: string) => void>;
}

/** One line that names the place a stage reads and opens its block. */
export const ReaderRow = component$<ReaderRowProps>((props) => (
  <MetadataRow label="Reads" labelWidth="w-28">
    <Badge tone={props.tone ?? 'neutral'} label={props.place} />
    <Text size="micro" tone="faint" class="flex-1">
      {props.detail}
    </Text>
    <Button
      variant="outline"
      size="sm"
      onClick$={$(() => props.onOpen$(props.block))}
    >
      Open
    </Button>
  </MetadataRow>
));

/** The places one stage reads, as a row per place that opens its values. */
export const ReadsRows = component$<{
  reads: PlaceRead[];
  onOpen$: QRL<(id: string) => void>;
}>((props) => (
  <>
    {props.reads.map((read) => (
      <ReaderRow
        key={read.block}
        place={read.place}
        detail={read.detail}
        tone={read.tone}
        block={read.block}
        onOpen$={props.onOpen$}
      />
    ))}
  </>
));

/** The block of the reader that runs one built in model. */
const READER_BLOCK: Record<LocalModelRole, RouterBlock> = {
  embeddings: 'embeddings',
  reranker: 'reranker',
  decision: 'decisionModel',
};

/** The built in model one reader of the flow runs. */
const READER_ROLE: Record<
  'embeddings' | 'reranker' | 'decisionModel',
  LocalModelRole
> = {
  embeddings: 'embeddings',
  reranker: 'reranker',
  decisionModel: 'decision',
};

/** How often the section asks for the state of a running download. */
const DOWNLOAD_POLL_MS = 800;

/**
 * One block of the router flow a reader can pick.
 *
 * A block is one of four kinds. A `stage` is a step of a turn, an `exit`
 * is where a turn leaves the chain, a `place` is where something runs, and
 * a `reader` is the first two joined: the thing a stage points at.
 *
 * A reader is the block that holds the choice, because the daemon really
 * holds it there. The model server has one address and two readers, a
 * chat model and an embedding model, which is why a stage points at a
 * reader rather than at the server: two stages read the same address with
 * two different models, and one block cannot hold both.
 */
type RouterBlock =
  | 'message'
  | 'deterministic'
  | 'retrieval'
  | 'decision'
  | 'extraction'
  | 'run'
  | 'builtin'
  | 'server'
  | 'lists'
  | 'embeddings'
  | 'reranker'
  | 'decisionModel'
  | 'spans'
  | 'chatModel'
  | 'embeddingModel'
  | 'refused'
  | 'asks'
  | 'memory';

/** The name of every block of the flow. */
const BLOCK_TITLES: Record<RouterBlock, string> = {
  message: 'The message',
  deterministic: 'Deterministic pass',
  retrieval: 'Retrieval',
  decision: 'Decision',
  extraction: 'Extraction',
  run: 'The run',
  builtin: 'This machine',
  server: 'Model server',
  lists: 'The lists',
  embeddings: 'Embedding model',
  reranker: 'Reranker',
  decisionModel: 'Decision model',
  spans: 'Spans',
  chatModel: 'Chat model',
  embeddingModel: 'Embedding model of the server',
  refused: 'No intent matched',
  asks: 'Asks for the value',
  memory: 'Long term memory',
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
    'Chooses one of the short list. The scores read the ranking alone and need no model; the reranker reads the message and every candidate with a built in model; Laya reads the short list as one typed choice and answers with one intent and a probability for each, without writing text; the language model reads only the short list, so the prompt stays small however large the catalog is.',
  extraction:
    'Reads the values of the intent that won. The lists need no model, the spans use the built in model, and the model reads every value.',
  run: 'The command of the intent runs with the values in place. A required value the router did not read stops the turn and asks the user for it instead.',
  builtin:
    'This machine runs the built in models in the daemon through ONNX Runtime, so they need no server and no network after the download. The device is one setting that every built in reader shares, so it lives here and not in each reader of its own.',
  server:
    'The address of the llama.cpp server. The readers that point here each name their own model, because one server answers a chat request and an embedding request with a different model, so the address is one setting and the models are three.',
  lists:
    'Reads a value out of the list an entity carries. A closed entity holds its values and a script entity answers with its own, and the rules of the matcher read both without a model of any kind. This is the reader that costs nothing at all.',
  embeddings:
    'Turns a message and an intent into vectors, so `launch the browser` reaches `open application` although the two share no word. The retrieval stage reads it for the whole catalog and the matcher of a list reads it for one value.',
  reranker:
    'Reads a message and one candidate together and reports how well the two fit, which is surer than a score of two separate vectors. It runs only on the short list the retrieval stage kept, so its cost belongs to one turn and not to the catalog.',
  decisionModel:
    'Answers a typed `choice` question over a state in one forward pass, without writing text. The decision stage asks it to choose among the short list and the extraction stage asks it one question per list of the intent that won, so both stages can point at this one reader and the cost is one forward pass each.',
  spans:
    'The built in GLiNER model finds the exact span of every label of the intent that won. A label carries no value when the model reads nothing, which leaves the value to the language model.',
  chatModel:
    'The model the server answers a chat request with. The decision stage reads the short list through it, the extraction stage reads every value through it, and a message no intent matched is answered through it.',
  embeddingModel:
    'The model the server answers an embedding request with. The retrieval stage and the matcher of a list read it, and they read the same one because the vectors of a catalog and the vectors of a value must come from one model.',
  refused:
    'The turn ends with no intent matched. Nothing runs. With the fallback on, the language model answers the message itself, with the earlier turns as its context; with it off, the reply names the reader that refused.',
  asks: 'The turn stops before the command and asks the user for the value the entity needs. The next message reaches the resolver with the asking turn as its history.',
  memory:
    'What the daemon learns about the user across the sessions. Only the turn that meets no intent reads it and only that turn teaches it, so the memory never bends the choice of an intent. The reader is a model of its own, and the write happens in the background, so no reply waits for it.',
};

/**
 * One reader of the flow, as the caller names it.
 *
 * A reader is a block of its own and not a value inside the block of a
 * stage, because one place serves several stages with a different model
 * for each. The server answers a chat request with one model and an
 * embedding request with another, so a stage points at the reader rather
 * than at the address.
 */
interface ReaderSpec {
  /** The id of the block. */
  id: RouterBlock;
  /** Whether the current settings point at this reader. */
  on: boolean;
  /** The name of the block. */
  title: string;
  /** One line about the model the reader runs. */
  detail: string;
  /** The state of the reader, as the block shows it. */
  label: string;
  /** The tone of the state. */
  tone: 'ok' | 'warn' | 'neutral' | 'error';
  /** The row of the reader in the reader column. */
  row: number;
}

/**
 * Read the readers of the flow into the blocks of the reader column.
 *
 * Every reader is drawn, whether the settings point at it or not, so a
 * link to a stage may be drawn from a reader the turn would not touch
 * yet. A reader the settings do not point at shows that it is unused
 * rather than leaving the graph, so the reader that answers a stage is
 * always one drag away.
 */
function readerNodes(readers: ReaderSpec[]): FlowNodeSpec[] {
  return readers.map((reader) => ({
    id: reader.id,
    title: reader.title,
    detail: reader.detail,
    status: reader.on
      ? { label: reader.label, tone: reader.tone }
      : { label: 'Not used', tone: 'neutral' },
    kind: 'place',
    column: 1,
    row: reader.row,
    // A reader always shows the point a connection leaves from and the
    // one it reaches, so a link to a stage may be drawn before the value
    // of the reader exists at all.
    ports: [
      { channel: 'setting', side: 'left' },
      { channel: 'setting', side: 'right' },
    ],
    dragFrom: true,
  }));
}

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
  /** The state of the librarian, or null while it loads. */
  librarianStatus: LibrarianStatusDto | null;
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
    const layaModel = useSignal(props.settings.routerLayaModel);
    const localDevice = useSignal(props.settings.routerLocalDevice);
    const phraseGate = useSignal(props.settings.routerPhraseGate);
    const listMatch = useSignal(props.settings.routerListMatch);
    const listFloor = useSignal(String(props.settings.routerListFloor));
    const fallbackLlm = useSignal(props.settings.routerFallbackLlm);
    const scriptFallback = useSignal(props.settings.routerScriptFallback);
    const openValuesLlm = useSignal(props.settings.routerOpenValuesLlm);
    const memoryEnabled = useSignal(props.settings.librarianEnabled);
    const memoryBaseUrl = useSignal(props.settings.librarianBaseUrl);
    const memoryModel = useSignal(props.settings.librarianModel);
    const responseQuality = useSignal(props.settings.responseQuality);
    const isCustom = useSignal(false);
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
    const applyingPreset = useSignal(false);
    const activeTab = useSignal<'performance' | 'flow'>('performance');

    const handleApplyPreset = $((quality: number) => {
      responseQuality.value = quality;
      isCustom.value = false;
      const preset = presetForQuality(quality, {
        routerRetrieve: retrieve.value as SettingsDto['routerRetrieve'],
        routerDecide: decide.value as SettingsDto['routerDecide'],
        routerExtract: extract.value as SettingsDto['routerExtract'],
        routerTopK: Number(topK.value) || props.settings.routerTopK,
        routerFloor: Number(floor.value) || props.settings.routerFloor,
        routerMargin: Number(margin.value) || props.settings.routerMargin,
        routerLexicalWeight:
          Number(lexicalWeight.value) || props.settings.routerLexicalWeight,
        routerDenseWeight:
          Number(denseWeight.value) || props.settings.routerDenseWeight,
        routerEmbedSource:
          embedSource.value as SettingsDto['routerEmbedSource'],
        routerEmbedModel: embedModel.value,
        routerEmbedLocalModel: embedLocalModel.value,
        routerRerankModel: rerankModel.value,
        routerLayaModel: layaModel.value,
        routerLocalDevice:
          localDevice.value as SettingsDto['routerLocalDevice'],
        routerPhraseGate: phraseGate.value,
        routerListMatch: listMatch.value as SettingsDto['routerListMatch'],
        routerListFloor:
          Number(listFloor.value) || props.settings.routerListFloor,
        routerFastPath: fastPath.value,
      } as SettingsDto);
      fastPath.value = preset.routerFastPath;
      retrieve.value = preset.routerRetrieve;
      decide.value = preset.routerDecide;
      extract.value = preset.routerExtract;
      topK.value = String(preset.routerTopK);
      floor.value = String(preset.routerFloor);
      margin.value = String(preset.routerMargin);
      lexicalWeight.value = String(preset.routerLexicalWeight);
      denseWeight.value = String(preset.routerDenseWeight);
      embedSource.value = preset.routerEmbedSource;
      embedLocalModel.value = preset.routerEmbedLocalModel;
      rerankModel.value = preset.routerRerankModel;
      localDevice.value = preset.routerLocalDevice;
      phraseGate.value = preset.routerPhraseGate;
      listMatch.value = preset.routerListMatch;
      listFloor.value = String(preset.routerListFloor);
      embedModel.value = preset.routerEmbedModel;
      props.onEdit$();
    });

    // Keep the quality signal where the stored settings left it, so a
    // saved change does not reset on reload. Speed 100 (quality 0) is
    // lexical/score/lists with no llama and the smallest model that fits
    // 4 GB.
    useTask$(({ track }) => {
      track(() => props.settings.responseQuality);
      responseQuality.value = props.settings.responseQuality;
      isCustom.value = false;
    });

    // When the flow is edited manually, the sliders no longer match the
    // preset for that quality. Show a warning and keep the flow live.
    useTask$(({ track }) => {
      track(() => responseQuality.value);
      track(() => retrieve.value);
      track(() => decide.value);
      track(() => extract.value);
      track(() => topK.value);
      track(() => floor.value);
      track(() => margin.value);
      track(() => lexicalWeight.value);
      track(() => denseWeight.value);
      track(() => embedSource.value);
      track(() => listMatch.value);
      track(() => listFloor.value);
      track(() => embedModel.value);
      track(() => embedLocalModel.value);
      track(() => rerankModel.value);
      track(() => layaModel.value);
      track(() => localDevice.value);
      track(() => phraseGate.value);
      track(() => fastPath.value);

      const live = {
        ...props.settings,
        routerRetrieve: retrieve.value as SettingsDto['routerRetrieve'],
        routerDecide: decide.value as SettingsDto['routerDecide'],
        routerExtract: extract.value as SettingsDto['routerExtract'],
        routerTopK: Number(topK.value) || props.settings.routerTopK,
        routerFloor: Number(floor.value) || props.settings.routerFloor,
        routerMargin: Number(margin.value) || props.settings.routerMargin,
        routerLexicalWeight:
          Number(lexicalWeight.value) || props.settings.routerLexicalWeight,
        routerDenseWeight:
          Number(denseWeight.value) || props.settings.routerDenseWeight,
        routerEmbedSource:
          embedSource.value as SettingsDto['routerEmbedSource'],
        routerEmbedLocalModel: embedLocalModel.value,
        routerRerankModel: rerankModel.value,
        routerLayaModel: layaModel.value,
        routerLocalDevice:
          localDevice.value as SettingsDto['routerLocalDevice'],
        routerPhraseGate: phraseGate.value,
        routerListMatch: listMatch.value as SettingsDto['routerListMatch'],
        routerListFloor:
          Number(listFloor.value) || props.settings.routerListFloor,
        routerEmbedModel: embedModel.value,
        routerFastPath: fastPath.value,
      } as SettingsDto;

      isCustom.value = isCustomForQuality(live, responseQuality.value);
    });

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
    //
    // Every file is configured in the one "Built in models" block, and the
    // stages only name the reader they run. The block therefore offers
    // every role the settings read at once, and the roles below are the
    // file each stage would open.
    const readsVectors =
      retrieve.value !== 'lexical' || listMatch.value !== 'lexical';
    const embedRoles: LocalModelRole[] =
      embedSource.value === 'local' && readsVectors ? ['embeddings'] : [];
    // The cross-encoder reranks the short list.
    const rerankRoles: LocalModelRole[] =
      decide.value === 'rerank' ? ['reranker'] : [];
    // The decision model scores the short list and, at the extraction
    // stage, chooses one value of every list the intent offers. Either
    // stage that asks it needs the same file on disk.
    const readsDecisionModel =
      decide.value === 'laya' || extract.value === 'laya';
    const decisionModelRoles: LocalModelRole[] = readsDecisionModel
      ? ['decision']
      : [];
    const localRoles: LocalModelRole[] = [
      ...embedRoles,
      ...rerankRoles,
      ...decisionModelRoles,
    ];
    const localChosen: Record<LocalModelRole, string> = {
      embeddings: embedLocalModel.value,
      reranker: rerankModel.value,
      decision: layaModel.value,
    };
    // The decision model of the reader each stage runs, named the way the
    // settings page names it.
    const localName = (role: LocalModelRole): string =>
      localStore?.models.find((entry) => entry.id === localChosen[role])
        ?.name ?? localChosen[role];
    const localInstalled = (role: LocalModelRole): boolean =>
      (localStore?.models ?? []).some(
        (entry) => entry.id === localChosen[role] && entry.installed,
      );
    /** The state of one built in reader, as its block shows it. */
    const localStoreLabel = (role: LocalModelRole): string =>
      localInstalled(role) ? 'On disk' : 'Missing';
    const localStoreTone = (role: LocalModelRole): 'ok' | 'warn' =>
      localInstalled(role) ? 'ok' : 'warn';

    /** Whether any stage asks the server for a chat answer. */
    const usesChat =
      decide.value === 'generative' ||
      extract.value === 'generative' ||
      fallbackLlm.value;

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
      extract.value === 'generative' ||
      fallbackLlm.value;
    const usesBuiltIn =
      (embedSource.value === 'local' && readsVectors) ||
      decide.value === 'rerank' ||
      decide.value === 'laya' ||
      extract.value === 'laya';

    // The places a stage reads, named the way the block that holds their
    // values names them. A stage shows one row per place, and the block of
    // that place holds the address, the file, and the device, so a
    // connection is written once rather than inside every stage that reads
    // it.
    const localRead = (role: LocalModelRole, detail: string): PlaceRead => ({
      block: READER_BLOCK[role],
      place: BLOCK_TITLES[READER_BLOCK[role]],
      detail,
      tone: localInstalled(role) ? 'ok' : 'warn',
    });
    const chatRead = (detail: string): PlaceRead => ({
      block: 'chatModel',
      place: BLOCK_TITLES.chatModel,
      detail,
      tone: llamaReachable ? 'ok' : 'error',
    });
    const embeddingRead = (detail: string): PlaceRead => ({
      block: 'embeddingModel',
      place: BLOCK_TITLES.embeddingModel,
      detail,
      tone: llamaReachable ? 'ok' : 'error',
    });
    const spansRead = (detail: string): PlaceRead => ({
      block: 'spans',
      place: BLOCK_TITLES.spans,
      detail,
      tone: gliner?.installed ? 'ok' : 'warn',
    });
    // The place the vectors come from is one setting, so the retrieval stage
    // and the matcher that reads a value of a list by meaning read the same
    // one.
    const vectorPlaces = (): PlaceRead[] =>
      readsVectors
        ? [
            embedSource.value === 'local'
              ? localRead(
                  'embeddings',
                  `The ${localName('embeddings')} file reads the vectors.`,
                )
              : embeddingRead(
                  `The ${embedModel.value} model at ${baseUrl.value} reads the vectors.`,
                ),
          ]
        : [];
    /** Fold the reads of one stage into one row per place. */
    const placesOf = (...groups: PlaceRead[][]): PlaceRead[] => {
      const kept: PlaceRead[] = [];
      for (const read of groups.flat()) {
        const held = kept.find((entry) => entry.block === read.block);
        if (!held) {
          kept.push({ ...read });
          continue;
        }
        held.detail = `${held.detail} ${read.detail}`;
        held.tone =
          held.tone === 'error' || read.tone === 'error'
            ? 'error'
            : held.tone === 'warn' || read.tone === 'warn'
              ? 'warn'
              : 'ok';
      }
      return kept;
    };
    /** The places the retrieval stage reads. */
    const retrievePlaces = (): PlaceRead[] => vectorPlaces();
    /** The places the decision stage reads. */
    const decidePlaces = (): PlaceRead[] => {
      if (decide.value === 'rerank') {
        return [
          localRead(
            'reranker',
            `The ${localName('reranker')} cross encoder ranks the short list.`,
          ),
        ];
      }
      if (decide.value === 'laya') {
        return [
          localRead(
            'decision',
            `The ${localName('decision')} decision model chooses one of the short list.`,
          ),
        ];
      }
      if (decide.value === 'generative') {
        return [
          chatRead(
            `The ${model.value} model at ${baseUrl.value} chooses one of the short list.`,
          ),
        ];
      }
      return [];
    };
    /** The places the extraction stage reads. */
    const extractPlaces = (): PlaceRead[] => {
      if (extract.value === 'spans') {
        return placesOf(
          [spansRead(`The ${chosenName} model finds the span of every label.`)],
          vectorPlaces(),
        );
      }
      if (extract.value === 'laya') {
        return placesOf(
          [
            localRead(
              'decision',
              `The ${localName('decision')} decision model chooses one value of every list.`,
            ),
          ],
          vectorPlaces(),
        );
      }
      if (extract.value === 'generative') {
        return placesOf(
          [
            chatRead(
              `The ${model.value} model at ${baseUrl.value} reads every value.`,
            ),
          ],
          vectorPlaces(),
        );
      }
      return placesOf(vectorPlaces());
    };

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
        column: 2,
        row: 0,
        stage: 1,
      },
      {
        id: 'deterministic',
        title: 'Deterministic pass',
        detail: 'proves what the catalog can prove',
        status: { label: fastPath.value ? 'On' : 'Off' },
        kind: 'stage',
        column: 2,
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
        column: 2,
        row: 2,
        stage: 3,
      },
      {
        id: 'decision',
        title: 'Decision',
        detail: 'chooses one of the short list, or none',
        status: { label: decideLabel },
        kind: 'stage',
        column: 2,
        row: 3,
        stage: 4,
      },
      {
        id: 'extraction',
        title: 'Extraction',
        detail: 'reads the values of the intent that won',
        status: { label: extractLabel },
        kind: 'stage',
        column: 2,
        row: 4,
        stage: 5,
      },
      {
        id: 'run',
        title: 'The run',
        detail: 'runs the command with the values in place',
        status: { label: 'Command' },
        kind: 'stage',
        column: 2,
        row: 5,
        stage: 6,
      },
      {
        id: 'refused',
        title: 'No intent matched',
        detail: fallbackLlm.value
          ? 'nothing runs, the model answers'
          : 'nothing runs, the reply names the reader',
        kind: 'exit',
        // An exit sits on the row of the stage it leaves, so its link runs
        // straight out of that stage. Stacking the exits below the chain
        // instead would make the two links cross, because the decision
        // leaves above the extraction and refuses below it.
        column: 3,
        row: 3,
      },
      {
        id: 'asks',
        title: 'Asks for the value',
        detail: 'a required value is missing',
        kind: 'exit',
        column: 3,
        row: 4,
      },
      {
        id: 'memory',
        title: 'Long term memory',
        detail: memoryModel.value,
        status: {
          label: memoryEnabled.value ? 'On' : 'Off',
          tone: memoryEnabled.value ? 'ok' : 'neutral',
        },
        // The memory is the step the refused branch takes before the model
        // answers it, so it stands on the row of that exit and one column
        // further out. It is always drawn, because the values of the memory
        // are set from this block.
        kind: 'exit',
        column: 4,
        row: 3,
      },
    ];

    // The places, then the readers that stand between a place and the
    // stages that read it. A reader is drawn only when a stage points at
    // it, so the graph never shows a model a turn would not touch.
    if (usesBuiltIn) {
      flowNodes.push({
        id: 'builtin',
        title: BLOCK_TITLES.builtin,
        detail: `${localRoles.length} model file(s) on one device`,
        status: {
          label: localDevice.value === 'auto' ? 'Auto' : localDevice.value,
        },
        kind: 'place',
        column: 0,
        row: 3,
      });
    }

    if (usesServer) {
      flowNodes.push({
        id: 'server',
        title: BLOCK_TITLES.server,
        detail: baseUrl.value,
        status: {
          label: llamaReachable ? 'Up' : 'Down',
          tone: llamaReachable ? 'ok' : 'error',
        },
        kind: 'place',
        column: 0,
        row: 4,
      });
    }

    flowNodes.push(
      ...readerNodes([
        {
          id: 'embeddings',
          on: readsVectors && embedSource.value === 'local',
          title: BLOCK_TITLES.embeddings,
          detail: localName('embeddings'),
          label: localStoreLabel('embeddings'),
          tone: localStoreTone('embeddings'),
          row: 0,
        },
        {
          id: 'embeddingModel',
          on: readsVectors && embedSource.value === 'server',
          title: BLOCK_TITLES.embeddingModel,
          detail: `${embedModel.value} at ${baseUrl.value}`,
          label: llamaReachable ? 'Up' : 'Down',
          tone: llamaReachable ? 'ok' : 'error',
          row: 1,
        },
        {
          id: 'reranker',
          on: decide.value === 'rerank',
          title: BLOCK_TITLES.reranker,
          detail: localName('reranker'),
          label: localStoreLabel('reranker'),
          tone: localStoreTone('reranker'),
          row: 2,
        },
        {
          id: 'decisionModel',
          on: readsDecisionModel,
          title: BLOCK_TITLES.decisionModel,
          detail: localName('decision'),
          label: localStoreLabel('decision'),
          tone: localStoreTone('decision'),
          row: 3,
        },
        {
          id: 'spans',
          on: extract.value === 'spans',
          title: BLOCK_TITLES.spans,
          detail: chosenName,
          label: gliner?.installed ? 'On disk' : 'Missing',
          tone: gliner?.installed ? 'ok' : 'warn',
          row: 4,
        },
        {
          id: 'chatModel',
          on: usesChat,
          title: BLOCK_TITLES.chatModel,
          detail: model.value,
          label: llamaReachable ? 'Up' : 'Down',
          tone: llamaReachable ? 'ok' : 'error',
          row: 5,
        },
        {
          id: 'lists',
          on: extract.value === 'lists',
          title: BLOCK_TITLES.lists,
          detail: 'no model at all',
          label: 'No model',
          tone: 'ok',
          row: 6,
        },
      ]),
    );

    // The route of the sentence that was read last, read back onto the
    // blocks of this graph. A place a stage read lights up only when the
    // reader that really answered it stands in the graph.
    const highlight = routeHighlight(preview.value, {
      vectors:
        retrieve.value !== 'lexical' && readsVectors
          ? embedSource.value === 'local'
            ? 'embeddings'
            : 'embeddingModel'
          : null,
      builtin:
        decide.value === 'rerank'
          ? 'reranker'
          : decide.value === 'laya' || extract.value === 'laya'
            ? 'decisionModel'
            : null,
      model: usesChat ? 'chatModel' : null,
      spans: extract.value === 'spans' ? 'spans' : null,
      // The memory answers only a refusal the model answers, so a turn that
      // keeps no memory and a refusal that runs nothing light no block.
      memory: memoryEnabled.value && fallbackLlm.value ? 'memory' : null,
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
      {
        id: 'refused-memory',
        from: 'refused',
        to: 'memory',
        label: 'a memory',
        kind: 'branch',
      },
    ];

    // A place supplies the readers that run on it, and a reader is what a
    // stage points at. The two hops exist because one place serves several
    // stages with a different model for each: the server answers a chat
    // request and an embedding request with two different models, so the
    // address is one block and the models are blocks of their own.
    if (usesBuiltIn) {
      if (readsVectors && embedSource.value === 'local') {
        flowEdges.push({
          id: 'builtin-embeddings',
          from: 'builtin',
          to: 'embeddings',
          label: 'one file',
          kind: 'setting',
        });
      }
      if (decide.value === 'rerank') {
        flowEdges.push({
          id: 'builtin-reranker',
          from: 'builtin',
          to: 'reranker',
          label: 'one file',
          kind: 'setting',
        });
      }
      if (readsDecisionModel) {
        flowEdges.push({
          id: 'builtin-decisionModel',
          from: 'builtin',
          to: 'decisionModel',
          label: 'one file',
          kind: 'setting',
        });
      }
      if (extract.value === 'spans') {
        flowEdges.push({
          id: 'builtin-spans',
          from: 'builtin',
          to: 'spans',
          label: 'one file',
          kind: 'setting',
        });
      }
    }

    if (usesServer) {
      if (readsVectors && embedSource.value === 'server') {
        flowEdges.push({
          id: 'server-embeddingModel',
          from: 'server',
          to: 'embeddingModel',
          label: 'the address',
          kind: 'setting',
        });
      }
      if (usesChat) {
        flowEdges.push({
          id: 'server-chatModel',
          from: 'server',
          to: 'chatModel',
          label: 'the address',
          kind: 'setting',
        });
      }
    }

    // The reader every stage points at. Each of these is one setting of the
    // flow, so a reader that is dropped on a stage is the same value the
    // stage panel holds.
    if (extract.value === 'lists') {
      flowEdges.push({
        id: 'lists-extraction',
        from: 'lists',
        to: 'extraction',
        label: 'the values',
        kind: 'setting',
      });
    }
    if (readsVectors && embedSource.value === 'local') {
      flowEdges.push({
        id: 'embeddings-retrieval',
        from: 'embeddings',
        to: 'retrieval',
        label: 'the vectors',
        kind: 'setting',
      });
    }
    if (readsVectors && embedSource.value === 'server') {
      flowEdges.push({
        id: 'embeddingModel-retrieval',
        from: 'embeddingModel',
        to: 'retrieval',
        label: 'the vectors',
        kind: 'setting',
      });
    }
    if (decide.value === 'rerank') {
      flowEdges.push({
        id: 'reranker-decision',
        from: 'reranker',
        to: 'decision',
        label: 'the choice',
        kind: 'setting',
      });
    }
    if (decide.value === 'laya') {
      flowEdges.push({
        id: 'decisionModel-decision',
        from: 'decisionModel',
        to: 'decision',
        label: 'the choice',
        kind: 'setting',
      });
    }
    if (extract.value === 'laya') {
      flowEdges.push({
        id: 'decisionModel-extraction',
        from: 'decisionModel',
        to: 'extraction',
        label: 'the values',
        kind: 'setting',
      });
    }
    if (extract.value === 'spans') {
      flowEdges.push({
        id: 'spans-extraction',
        from: 'spans',
        to: 'extraction',
        label: 'the values',
        kind: 'setting',
      });
    }
    if (decide.value === 'generative') {
      flowEdges.push({
        id: 'chatModel-decision',
        from: 'chatModel',
        to: 'decision',
        label: 'the choice',
        kind: 'setting',
      });
    }
    if (extract.value === 'generative') {
      flowEdges.push({
        id: 'chatModel-extraction',
        from: 'chatModel',
        to: 'extraction',
        label: 'the values',
        kind: 'setting',
      });
    }
    if (fallbackLlm.value) {
      flowEdges.push({
        id: 'chatModel-refused',
        from: 'chatModel',
        to: 'refused',
        label: 'the words',
        kind: 'setting',
      });
    }

    const handleSelectBlock = $((id: string) => {
      selectedBlock.value = id as RouterBlock;
      panelOpen.value = true;
    });

    const handleClosePanel = $(() => {
      panelOpen.value = false;
    });

    /**
     * Join one output point of a block to the input point of another and
     * write the one setting the link stands for. A pair that names no
     * setting of the router is left alone, so a reader may drop a link
     * anywhere without changing a value by accident.
     */
    const handleConnect = $((from: string, to: string) => {
      switch (`${from}->${to}`) {
        case 'chatModel->decision':
          decide.value = 'generative';
          break;
        case 'chatModel->extraction':
          extract.value = 'generative';
          break;
        case 'chatModel->refused':
          fallbackLlm.value = true;
          break;
        case 'decisionModel->decision':
          decide.value = 'laya';
          break;
        case 'decisionModel->extraction':
          extract.value = 'laya';
          break;
        case 'reranker->decision':
          decide.value = 'rerank';
          break;
        case 'spans->extraction':
          extract.value = 'spans';
          break;
        case 'lists->extraction':
          extract.value = 'lists';
          break;
        case 'embeddings->retrieval':
          retrieve.value = 'dense';
          embedSource.value = 'local';
          break;
        case 'embeddingModel->retrieval':
          retrieve.value = 'dense';
          embedSource.value = 'server';
          break;
        default:
          return;
      }
      props.onEdit$();
    });

    const handleChooseLocalModel = $((role: LocalModelRole, id: string) => {
      if (role === 'embeddings') {
        embedLocalModel.value = id;
      } else if (role === 'decision') {
        layaModel.value = id;
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
        // The sliders are the source of truth for speed/quality and the flow
        // is a lerp from them: 100 speed is lexical/score/lists with no
        // llama and the smallest model that fits 4 GB.
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
        routerLayaModel: layaModel.value,
        routerLocalDevice: localDevice.value,
        routerPhraseGate: phraseGate.value,
        routerListMatch: listMatch.value,
        routerListFloor: Number(listFloor.value),
        routerFallbackLlm: fallbackLlm.value,
        routerScriptFallback: scriptFallback.value,
        routerOpenValuesLlm: openValuesLlm.value,
        // The memory of the daemon belongs to the branch that no intent
        // reaches, so the flow carries its values with the values of the
        // turn rather than in a form of its own.
        librarianEnabled: memoryEnabled.value,
        librarianBaseUrl: memoryBaseUrl.value,
        librarianModel: memoryModel.value,
        responseQuality: responseQuality.value,
        responseSpeed: 100 - responseQuality.value,
      });
    });

    // The flow fills the room the section leaves and the values of the
    // picked step take a panel of their own beside it, so a pick shifts the
    // card rather than nesting a second surface in it, and the page never
    // scrolls a pipeline that is taller than the screen.
    return (
      <Stack gap="md" class="flex flex-1 min-h-0 flex-col overflow-hidden">
        <Box class="sticky top-0 z-10 -mx-8 flex items-center justify-between gap-4 border-b border-ds-line bg-ds-bg px-8 py-3 overflow-hidden">
          <Tabs
            ariaLabel="Resolver view"
            wrap
            items={[
              { id: 'performance', label: 'Performance' },
              { id: 'flow', label: 'Flow' },
            ]}
            selected={activeTab.value}
            onSelect$={$((id: string) => {
              activeTab.value = id as 'performance' | 'flow';
            })}
          />
          <Button
            variant="solid"
            size="md"
            disabled={props.pending || !canStore}
            onClick$={handleSave}
          >
            {props.pending ? 'Saving...' : 'Save resolver'}
          </Button>
        </Box>

        {storeReason ? (
          <Alert
            tone="error"
            title="The settings cannot be stored"
            message={storeReason}
          />
        ) : null}

        <div
          hidden={activeTab.value !== 'performance'}
          class="flex flex-1 min-h-0 flex-col overflow-auto"
        >
          <Stack gap="md" class="flex min-h-0 flex-col">
            <SystemSlidersSection
              settings={props.settings}
              quality={responseQuality.value}
              isCustom={isCustom.value}
              onQualityChange$={handleApplyPreset}
              disabled={props.pending || !canStore || applyingPreset.value}
            />
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
          </Stack>
        </div>
        <div
          hidden={activeTab.value !== 'flow'}
          class="flex flex-1 min-h-0 flex-col overflow-hidden"
        >
          <Box class="flex flex-1 min-h-0 flex-col gap-6 overflow-hidden lg:flex-row">
            <Card
              class="flex flex-1 min-h-0 w-full min-w-0 flex-col overflow-hidden"
              bodyClass="flex flex-1 min-h-0 flex-col overflow-hidden"
            >
              <Stack gap="md" class="flex flex-1 min-h-0 overflow-hidden">
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
                      The message passes through the stages below and stops at
                      the first stage that can answer. Every stage is chosen in
                      the flow itself, and every stage has a reader that needs
                      no model, so a server that is down costs accuracy rather
                      than the turn.
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
                    onConnect$={handleConnect}
                  />

                  {highlight ? (
                    <Stack
                      direction="row"
                      gap="sm"
                      align="center"
                      justify="between"
                    >
                      <Text size="hud" tone="faint">
                        The flow shows the route of the sentence that was read
                        last.
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

                      {readsVectors ? (
                        <ReadsRows
                          reads={retrievePlaces()}
                          onOpen$={handleSelectBlock}
                        />
                      ) : (
                        <Text size="hud" tone="faint">
                          The words of the catalog need no model, so this stage
                          reads nothing here.
                        </Text>
                      )}
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

                    {decidePlaces().length > 0 ? (
                      <ReadsRows
                        reads={decidePlaces()}
                        onOpen$={handleSelectBlock}
                      />
                    ) : (
                      <Text size="hud" tone="faint">
                        The scores read the ranking alone, so this stage needs
                        no model.
                      </Text>
                    )}

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

                    {extractPlaces().length > 0 ? (
                      <ReadsRows
                        reads={extractPlaces()}
                        onOpen$={handleSelectBlock}
                      />
                    ) : (
                      <Text size="hud" tone="faint">
                        The rules of the matcher need no model, so this stage
                        reads only what the message spells out.
                      </Text>
                    )}

                    <PanelGroup
                      title="Reading a value of a list"
                      hint="The rules of the matcher read the spelling of a value; the embeddings read the meaning, so `the mail client` reaches `thunderbird`. Either reader fills a value the reader above left empty."
                    >
                      <ChoiceRow
                        label="Reader"
                        note="The rules run first and the embeddings follow for what the rules could not read."
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
                    </PanelGroup>

                    <ToggleSwitch
                      label="Read open values with the model"
                      hint="An open value is the words the user said, and the built in reader reads most of them in a few milliseconds. With this on, the language model reads a value no built in reader found, which costs about two seconds; with it off, the turn asks the user for the value instead."
                      checked={openValuesLlm.value}
                      disabled={props.pending || !canStore}
                      onChange$={$((next: boolean) => {
                        openValuesLlm.value = next;
                        props.onEdit$();
                      })}
                    />
                  </>
                ) : null}

                {selectedBlock.value === 'run' ? (
                  <Text size="micro" tone="faint">
                    The command of the intent runs with the values the stages
                    above read. Every value is substituted into the command in
                    place of its token. A required entity the extraction stage
                    did not read stops the turn here and takes the branch `Asks
                    for the value` instead.
                  </Text>
                ) : null}

                {selectedBlock.value === 'asks' ? (
                  <Text size="micro" tone="faint">
                    The turn stops before the command and asks the user for the
                    value the entity needs. The asking turn reaches the resolver
                    as the history of the next message, so a value alone is
                    enough to answer it. A value the deterministic pass proves
                    from the catalog needs no question.
                  </Text>
                ) : null}

                {selectedBlock.value === 'refused' ? (
                  <>
                    <ChoiceRow
                      label="Answer with the language model"
                      note="On answers a message no intent matched with the language model, using the earlier turns as context. Off replies that no intent matched. The daemon runs no command either way."
                      value={fallbackLlm.value ? 'on' : 'off'}
                      options={SWITCHES}
                      disabled={props.pending || !canStore}
                      onPick$={$((next: string) => {
                        fallbackLlm.value = next === 'on';
                        props.onEdit$();
                      })}
                    />

                    {fallbackLlm.value ? (
                      <ChoiceRow
                        label="Write a script when words are not enough"
                        note="When the message asks for a task, the model may write one shell script against the commands of the machine instead of an answer in words. The daemon runs no script without your approval, and it rates how rough the script is on the machine."
                        value={scriptFallback.value ? 'on' : 'off'}
                        options={SWITCHES}
                        disabled={props.pending || !canStore}
                        onPick$={$((next: string) => {
                          scriptFallback.value = next === 'on';
                          props.onEdit$();
                        })}
                      />
                    ) : null}

                    {fallbackLlm.value ? (
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
                  </>
                ) : null}

                {selectedBlock.value === 'memory' ? (
                  <>
                    <ToggleSwitch
                      label="Keep a memory"
                      hint="Off, the daemon writes no episode and no memory reaches the answer of a turn that met no intent."
                      checked={memoryEnabled.value}
                      disabled={props.pending || !canStore}
                      onChange$={$((next: boolean) => {
                        memoryEnabled.value = next;
                        props.onEdit$();
                      })}
                    />

                    <Stack gap="xs">
                      <FieldLabel
                        label="Memory server"
                        hint="The server the memory reads a whole turn with, in the OpenAI compatible shape. It may be the server above or a second one, so the memory reads a larger model than the turn does."
                      />
                      <TextInput
                        kind="input"
                        surface="field"
                        name="librarianBaseUrl"
                        ariaLabel="Address of the memory server"
                        placeholder="http://127.0.0.1:8012/v1"
                        value={memoryBaseUrl.value}
                        disabled={props.pending || !canStore}
                        onInput$={$((event: Event) => {
                          memoryBaseUrl.value = (
                            event.target as HTMLInputElement
                          ).value;
                          props.onEdit$();
                        })}
                      />
                    </Stack>

                    <Stack gap="xs">
                      <FieldLabel
                        label="Memory model"
                        hint="The model the memory server answers to."
                      />
                      <TextInput
                        kind="input"
                        surface="field"
                        name="librarianModel"
                        ariaLabel="Model of the memory server"
                        placeholder="qwen3.5-4b"
                        value={memoryModel.value}
                        disabled={props.pending || !canStore}
                        onInput$={$((event: Event) => {
                          memoryModel.value = (
                            event.target as HTMLInputElement
                          ).value;
                          props.onEdit$();
                        })}
                      />
                    </Stack>

                    {props.librarianStatus ? (
                      <MetadataRow label="Reader" labelWidth="w-28">
                        <Badge
                          tone={props.librarianStatus.reachable ? 'ok' : 'warn'}
                          label={
                            props.librarianStatus.reachable ? 'Up' : 'Down'
                          }
                        />
                        <Text size="micro" tone="faint">
                          {props.librarianStatus.detail ??
                            props.librarianStatus.baseUrl}
                        </Text>
                      </MetadataRow>
                    ) : null}

                    <Text size="micro" tone="faint">
                      The concepts the memory holds, and the linter that sweeps
                      them, stand in the Librarian section.
                    </Text>
                  </>
                ) : null}

                {selectedBlock.value === 'builtin' ? (
                  <>
                    <LocalModelsSection
                      store={localStore}
                      roles={[]}
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

                    <Text size="micro" tone="faint">
                      Every built in reader runs on this one device, so the
                      device is set here and not inside each of the readers. The
                      file of a reader is set in the block of that reader.
                      {localRoles.length > 0
                        ? ` ${localRoles.length} built in reader(s) run here right now.`
                        : ' No stage reads a built in model right now, so no file is needed.'}
                    </Text>
                  </>
                ) : null}

                {selectedBlock.value === 'embeddings' ||
                selectedBlock.value === 'reranker' ||
                selectedBlock.value === 'decisionModel' ? (
                  <LocalModelsSection
                    store={localStore}
                    roles={[READER_ROLE[selectedBlock.value]]}
                    chosen={localChosen}
                    device={localDevice.value}
                    devices={gliner?.devices ?? []}
                    cudaBuild={gliner?.cudaBuild ?? false}
                    pending={props.pending}
                    installing={props.installing}
                    showDevice={false}
                    onChoose$={handleChooseLocalModel}
                    onDevice$={handleLocalDevice}
                    onDownload$={props.onDownloadLocalModel$}
                    onRemove$={props.onRemoveLocalModel$}
                  />
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
                      showModel={false}
                      onBaseUrl$={handleBaseUrl}
                      onModel$={handleModel}
                      budget={props.status?.budget ?? null}
                    />

                    <Text size="micro" tone="faint">
                      The address is one setting and the models are three: a
                      chat model and an embedding model, each of which has a
                      block of its own, because one server answers a chat
                      request and an embedding request with a different model.
                    </Text>
                  </>
                ) : null}

                {selectedBlock.value === 'chatModel' ? (
                  <ServerFields
                    baseUrl={baseUrl.value}
                    model={model.value}
                    modelOptions={modelOptions}
                    resolvedBaseUrl={dependencies?.llama.baseUrl ?? null}
                    canStore={canStore}
                    llamaReachable={llamaReachable}
                    pending={props.pending}
                    showBaseUrl={false}
                    onBaseUrl$={handleBaseUrl}
                    onModel$={handleModel}
                    budget={props.status?.budget ?? null}
                  />
                ) : null}

                {selectedBlock.value === 'embeddingModel' ? (
                  <Stack gap="xs">
                    <FieldLabel
                      label="Embedding model"
                      hint={`The model this reader asks ${baseUrl.value} for, at its embedding path. The retrieval stage and the matcher of a list read it, and they read the same one, because the vectors of a catalog and the vectors of a value must come from one model. The server offers its embedding models at ${
                        dependencies?.llama.baseUrl ?? 'its address'
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
                ) : null}

                {selectedBlock.value === 'spans' && gliner ? (
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

                {selectedBlock.value === 'lists' ? (
                  <Text size="micro" tone="faint">
                    This reader needs no model and no server. A closed entity
                    holds its own values and a script entity answers with its
                    own list, and the rules of the matcher read both. Here is
                    nothing to set, which is also why nothing here can fail.
                  </Text>
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
                    <Alert
                      tone="error"
                      title="Save failed"
                      message={props.error}
                    />
                  ) : null}

                  {props.saved && !props.error ? (
                    <Alert
                      tone="info"
                      title="Saved"
                      message="The daemon uses these values for the next message."
                    />
                  ) : null}
                </Stack>
              </Box>
            </SidePanel>
          </Box>
        </div>
      </Stack>
    );
  },
);
