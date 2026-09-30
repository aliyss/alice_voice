/**
 * The settings route.
 *
 * The loader reads the settings and the intent configuration over REST.
 * The default component defines the handles that wrap the `server$`
 * functions and renders the page component. The file holds no page logic.
 */
import type { DocumentHead } from '@builder.io/qwik-city';

import type { SettingsPageData } from '~/components/pages/settings-page';
import type {
  MemoryConceptOutcome,
  MemoryDeleteOutcome,
} from '~/components/sections/settings/memory-search-section';

import type {
  DependenciesDto,
  IntentDto,
  IntentListDto,
  LibrarianStatusDto,
  MemoryNodeDto,
  MemoryQueryDto,
  MemoryWriteRequestDto,
  ResolverStatusDto,
  SettingsDto,
} from '~/types/dto';

import type { IntentInput } from '~/schemas/intent';
import type { SettingsInput } from '~/schemas/settings';

import { $, component$ } from '@builder.io/qwik';
import { routeLoader$ } from '@builder.io/qwik-city';

import { readSettingsSection } from '~/components/pages/settings-page';
import { SettingsPage } from '~/components/pages/settings-page';

import { backendGet } from '~/lib/backend-client';

import { getDependencies } from '~/api/dependencies';
import {
  addExampleIntents,
  createIntent,
  deleteIntent,
  previewScript,
  updateIntent,
} from '~/api/intents';
import {
  deleteConcept,
  getLibrarianStatus,
  lintMemory,
  mergeConcept,
  queryMemory,
  retireFact,
  writeMemory,
} from '~/api/librarian';
import {
  deleteGlinerModel,
  deleteLocalModel,
  downloadGlinerModel,
  downloadLocalModel,
  getResolverStatus,
  previewMessage,
} from '~/api/resolver';
import { updateSettings } from '~/api/settings';

/** The path of the settings endpoint. */
const SETTINGS_PATH = '/api/v1/settings';

/** The path of the intent endpoint. */
const INTENTS_PATH = '/api/v1/intents';

/** The path of the resolver endpoint. */
const RESOLVER_PATH = '/api/v1/resolver';

/** The path of the dependency endpoint. */
const DEPENDENCIES_PATH = '/api/v1/dependencies';

/** The path of the librarian endpoint. */
const LIBRARIAN_PATH = '/api/v1/librarian';

/** The default settings when the daemon does not answer. */
const FALLBACK_SETTINGS: SettingsDto = {
  queueEnabled: true,
  resolverBackend: 'router',
  resolverBaseUrl: 'http://127.0.0.1:8012/v1',
  resolverModel: 'qwen3.5-4b',
  glinerModel: 'gliner_small-v2.1',
  glinerDevice: 'auto',
  glinerThreshold: 0.3,
  routerFastPath: true,
  routerRetrieve: 'lexical',
  routerDecide: 'generative',
  routerExtract: 'spans',
  routerTopK: 8,
  routerFloor: 0.45,
  routerMargin: 0.1,
  routerLexicalWeight: 1,
  routerDenseWeight: 1,
  routerEmbedModel: 'bge-small-en-v1.5',
  routerModelsDir: 'models/router',
  routerEmbedSource: 'server',
  routerEmbedLocalModel: 'bge-small-en-v1.5',
  routerRerankModel: 'ms-marco-MiniLM-L-6-v2',
  routerLayaModel: 'laya',
  routerLocalDevice: 'auto',
  routerPhraseGate: true,
  routerListMatch: 'lexical',
  routerListFloor: 0.8,
  routerFallbackLlm: true,
  routerScriptFallback: true,
  routerOpenValuesLlm: true,
  responseQuality: 50,
  responseSpeed: 50,
  previewSentences: [],
  librarianEnabled: true,
  librarianBaseUrl: 'http://127.0.0.1:8012/v1',
  librarianModel: 'qwen3.5-4b',
};

/** Read the stored settings, or null when the database does not answer. */
async function readSettings(): Promise<SettingsDto | null> {
  try {
    return await backendGet<SettingsDto>(SETTINGS_PATH);
  } catch {
    return null;
  }
}

/** Read the stored intents, or an empty list. */
async function readIntents(): Promise<IntentDto[]> {
  try {
    const list = await backendGet<IntentListDto>(INTENTS_PATH);
    return list.items;
  } catch {
    return [];
  }
}

/**
 * Read the places the settings depend on, or null.
 *
 * The reply needs no database, so the page can tell a database that is
 * down from a daemon that is down.
 */
async function readDependencies(): Promise<DependenciesDto | null> {
  try {
    return await backendGet<DependenciesDto>(DEPENDENCIES_PATH);
  } catch {
    return null;
  }
}

/** Read the state of the resolver, or null. */
async function readResolverStatus(): Promise<ResolverStatusDto | null> {
  try {
    return await backendGet<ResolverStatusDto>(RESOLVER_PATH);
  } catch {
    return null;
  }
}

/** Read the state of the librarian, or null. */
async function readLibrarianStatus(): Promise<LibrarianStatusDto | null> {
  try {
    return await backendGet<LibrarianStatusDto>(LIBRARIAN_PATH);
  } catch {
    return null;
  }
}

/**
 * Read the concepts the memory holds, or an empty list.
 *
 * The request names no words, so the daemon answers with the concepts it
 * changed last. The cloud and the list of the librarian section open on
 * them, so a reader sees the memory without searching for it first.
 */
async function readMemories(): Promise<MemoryNodeDto[]> {
  try {
    const list = await backendGet<MemoryQueryDto>(`${LIBRARIAN_PATH}/memories`);
    return list.nodes;
  } catch {
    return [];
  }
}

/**
 * Read the settings, the intent configuration, and the resolver state.
 *
 * A database that does not answer leaves the stored values unknown, so
 * the page shows the values the daemon starts from and disables the
 * writes.
 */
export const useSettings = routeLoader$(
  async ({ url }): Promise<SettingsPageData> => {
    const dependencies = await readDependencies();
    const settings =
      (await readSettings()) ?? dependencies?.configured ?? FALLBACK_SETTINGS;
    const intents = await readIntents();
    const status = await readResolverStatus();
    const librarianStatus = await readLibrarianStatus();
    const memories = await readMemories();
    // The address names the section, so a link opens the surface where it
    // is about to point.
    const section = readSettingsSection(url.searchParams.get('section'));
    return {
      section,
      settings,
      intents,
      status,
      dependencies,
      librarianStatus,
      memories,
    };
  },
);

export default component$(() => {
  const data = useSettings();

  const handleUpdateSettings = $((input: SettingsInput) =>
    updateSettings(input),
  );
  const handleCreateIntent = $((input: IntentInput) => createIntent(input));
  const handleUpdateIntent = $((payload: { id: string; intent: IntentInput }) =>
    updateIntent(payload),
  );
  const handleDeleteIntent = $((id: string) => deleteIntent(id));
  const handleAddExampleIntents = $(() => addExampleIntents());
  const handlePreviewScript = $((script: string) => previewScript(script));
  const handlePreviewMessage = $((text: string) => previewMessage(text));
  const handleReadResolverStatus = $(() => getResolverStatus());
  const handleReadDependencies = $(() => getDependencies());
  const handleDownloadModel = $((id: string) => downloadGlinerModel(id));
  const handleRemoveModel = $((id: string) => deleteGlinerModel(id));
  const handleDownloadLocalModel = $((id: string) => downloadLocalModel(id));
  const handleRemoveLocalModel = $((id: string) => deleteLocalModel(id));
  const handleReadLibrarianStatus = $(() => getLibrarianStatus());
  const handleSearchMemory = $(async (text: string) => {
    const result = await queryMemory({ text });
    if (result.failed) {
      return { failed: true, message: result.message, nodes: [] };
    }
    return { failed: false, nodes: result.data.nodes };
  });
  const handleLintMemory = $(async () => {
    const result = await lintMemory();
    if (result.failed) {
      return {
        failed: true,
        message: result.message,
        duplicates: [],
        orphans: [],
        closed: [],
      };
    }
    return {
      failed: false,
      duplicates: result.data.duplicates,
      orphans: result.data.orphans,
      closed: result.data.closed,
    };
  });
  const handleSaveConcept = $(
    async (input: MemoryWriteRequestDto): Promise<MemoryConceptOutcome> => {
      const result = await writeMemory(input);
      if (result.failed) {
        return { failed: true, message: result.message, node: null };
      }
      return { failed: false, node: result.data };
    },
  );
  const handleRetireFact = $(
    async (id: string): Promise<MemoryConceptOutcome> => {
      const result = await retireFact(id);
      if (result.failed) {
        return { failed: true, message: result.message, node: null };
      }
      return { failed: false, node: result.data };
    },
  );
  const handleMergeConcept = $(
    async (id: string, into: string): Promise<MemoryConceptOutcome> => {
      const result = await mergeConcept(id, into);
      if (result.failed) {
        return { failed: true, message: result.message, node: null };
      }
      return { failed: false, node: result.data };
    },
  );
  const handleDeleteConcept = $(
    async (id: string): Promise<MemoryDeleteOutcome> => {
      const result = await deleteConcept(id);
      if (result.failed) {
        return { failed: true, message: result.message };
      }
      return { failed: false };
    },
  );

  return (
    <SettingsPage
      section={data.value.section}
      settings={data.value.settings}
      intents={data.value.intents}
      status={data.value.status}
      librarianStatus={data.value.librarianStatus}
      memories={data.value.memories}
      dependencies={data.value.dependencies}
      onUpdateSettings$={handleUpdateSettings}
      onSearchMemory$={handleSearchMemory}
      onSaveConcept$={handleSaveConcept}
      onRetireFact$={handleRetireFact}
      onMergeConcept$={handleMergeConcept}
      onDeleteConcept$={handleDeleteConcept}
      onLintMemory$={handleLintMemory}
      onReadLibrarianStatus$={handleReadLibrarianStatus}
      onCreateIntent$={handleCreateIntent}
      onUpdateIntent$={handleUpdateIntent}
      onDeleteIntent$={handleDeleteIntent}
      onAddExampleIntents$={handleAddExampleIntents}
      onPreviewScript$={handlePreviewScript}
      onPreviewMessage$={handlePreviewMessage}
      onReadResolverStatus$={handleReadResolverStatus}
      onReadDependencies$={handleReadDependencies}
      onDownloadModel$={handleDownloadModel}
      onRemoveModel$={handleRemoveModel}
      onDownloadLocalModel$={handleDownloadLocalModel}
      onRemoveLocalModel$={handleRemoveLocalModel}
    />
  );
});

export const head: DocumentHead = {
  title: 'Settings — Alice Voice',
  meta: [
    {
      name: 'description',
      content:
        'Control the queue, the intent resolver, the memory, and the intents of the Alice Voice daemon.',
    },
  ],
};
