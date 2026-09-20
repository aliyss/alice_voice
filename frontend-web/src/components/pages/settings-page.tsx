/**
 * `SettingsPage` is the settings surface of the daemon.
 *
 * The surface holds a section for the queue, one for the intent resolver,
 * and one for the intent configuration. A bar of sections stays beside the
 * content, so a reader moves between them without scrolling through the
 * ones they do not need.
 *
 * The page receives every action as a callback prop, so the route file
 * defines the handles that wrap the `server$` functions.
 */
import type { QRL } from '@builder.io/qwik';

import type { ApiResult } from '~/types/bridge';
import type {
  DependenciesDto,
  IntentDto,
  IntentListDto,
  ResolverStatusDto,
  SettingsDto,
} from '~/types/dto';

import type { ScriptPreviewResult } from '~/api/intents';
import type { ResolverPreviewResult } from '~/api/resolver';

import type { IntentInput } from '~/schemas/intent';
import type { ResolverSettingsInput, SettingsInput } from '~/schemas/settings';

import { $, component$, useSignal } from '@builder.io/qwik';

import { IntentFormSection } from '~/components/sections/settings/intent-form-section';
import { IntentListSection } from '~/components/sections/settings/intent-list-section';
import { QueueToggleSection } from '~/components/sections/settings/queue-toggle-section';
import { ResolverSettingsSection } from '~/components/sections/settings/resolver-settings-section';
import { Alert } from '~/components/ui/alert';
import { Box } from '~/components/ui/box';
import { PageHeader } from '~/components/ui/page-header';
import { ScrollArea } from '~/components/ui/scroll-area';
import { SectionNav } from '~/components/ui/section-nav';
import { SidePanel } from '~/components/ui/side-panel';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The sections of the surface, in the order the bar shows them. */
const SECTIONS = ['resolver', 'queue', 'intents'] as const;

/** One section of the settings surface. */
type SettingsSection = (typeof SECTIONS)[number];

/** The name and the one line of every section of the surface. */
const SECTION_TEXT: Record<SettingsSection, { title: string; note: string }> = {
  resolver: {
    title: 'Intent resolver',
    note: 'The engine that reads the intent of a message, the flow it passes through, and the values every step reads.',
  },
  queue: {
    title: 'Queue',
    note: 'Whether the daemon stores every turn in the database.',
  },
  intents: {
    title: 'Intents',
    note: 'The intents the resolver chooses among, the entities they read, and the commands they run.',
  },
};

/**
 * Read the section the address asks for.
 *
 * The address names the section, so a link opens the surface on the
 * section it is about. A name the surface does not know opens the first
 * section.
 */
export function readSettingsSection(value: string | null | undefined): string {
  return SECTIONS.find((entry) => entry === value) ?? 'resolver';
}

/** The props of `SettingsPage`. */
export interface SettingsPageProps {
  /** The section the surface opens on. */
  section: string;
  /** The stored settings of the daemon. */
  settings: SettingsDto;
  /** The stored intents, in name order. */
  intents: IntentDto[];
  /** The state of the intent resolver, or null when the daemon did not answer. */
  status: ResolverStatusDto | null;
  /**
   * The places the settings depend on, or null when the daemon did not
   * answer. A section keeps its controls disabled while the place that
   * stores its value does not answer.
   */
  dependencies: DependenciesDto | null;
  /**
   * Update the settings. The route file defines the handle and passes it
   * as this callback prop.
   */
  onUpdateSettings$: QRL<
    (input: SettingsInput) => Promise<ApiResult<SettingsDto>>
  >;
  /** Create one intent. */
  onCreateIntent$: QRL<
    (input: IntentInput) => Promise<ApiResult<IntentListDto>>
  >;
  /** Replace one intent. */
  onUpdateIntent$: QRL<
    (payload: {
      id: string;
      intent: IntentInput;
    }) => Promise<ApiResult<IntentListDto>>
  >;
  /** Delete one intent. */
  onDeleteIntent$: QRL<(id: string) => Promise<ApiResult<IntentListDto>>>;
  /** Add the example intents the catalog does not hold yet. */
  onAddExampleIntents$: QRL<() => Promise<ApiResult<IntentListDto>>>;
  /** Run one script of an intent and return the values it answers with. */
  onPreviewScript$: QRL<(script: string) => Promise<ScriptPreviewResult>>;
  /** Read one sentence without running anything and return its route. */
  onPreviewMessage$: QRL<(text: string) => Promise<ResolverPreviewResult>>;
  /** Read the state of the resolver again. */
  onReadResolverStatus$: QRL<() => Promise<ApiResult<ResolverStatusDto>>>;
  /** Read the state of the places the settings depend on again. */
  onReadDependencies$: QRL<() => Promise<ApiResult<DependenciesDto>>>;
  /** Download one built in GLiNER model. */
  onDownloadModel$: QRL<(id: string) => Promise<ApiResult<ResolverStatusDto>>>;
  /** Remove the files of one built in GLiNER model. */
  onRemoveModel$: QRL<(id: string) => Promise<ApiResult<ResolverStatusDto>>>;
  /** Download one built in model of the router. */
  onDownloadLocalModel$: QRL<
    (id: string) => Promise<ApiResult<ResolverStatusDto>>
  >;
  /** Remove the files of one built in model of the router. */
  onRemoveLocalModel$: QRL<
    (id: string) => Promise<ApiResult<ResolverStatusDto>>
  >;
}

/** The loader data of the page. The callback props are excluded. */
export type SettingsPageData = Omit<
  SettingsPageProps,
  | 'onUpdateSettings$'
  | 'onCreateIntent$'
  | 'onUpdateIntent$'
  | 'onDeleteIntent$'
  | 'onPreviewScript$'
  | 'onPreviewMessage$'
  | 'onAddExampleIntents$'
  | 'onReadResolverStatus$'
  | 'onReadDependencies$'
  | 'onDownloadModel$'
  | 'onRemoveModel$'
  | 'onDownloadLocalModel$'
  | 'onRemoveLocalModel$'
>;

export const SettingsPage = component$<SettingsPageProps>((props) => {
  const section = useSignal<SettingsSection>(
    readSettingsSection(props.section) as SettingsSection,
  );
  const settings = useSignal(props.settings);
  const intents = useSignal(props.intents);
  const status = useSignal(props.status);
  const dependencies = useSignal(props.dependencies);
  const installing = useSignal(false);
  const editingId = useSignal<string | null>(null);
  const formOpen = useSignal(false);
  const pending = useSignal(false);
  const queueError = useSignal<string | null>(null);
  const resolverError = useSignal<string | null>(null);
  const resolverSaved = useSignal(false);
  const intentError = useSignal<string | null>(null);

  const editing =
    intents.value.find((intent) => intent.id === editingId.value) ?? null;

  // The database is the place that stores every setting on this page, so
  // a database that does not answer disables every write.
  const canStore = dependencies.value?.database.reachable === true;
  const storeReason = canStore
    ? null
    : dependencies.value
      ? `The ${dependencies.value.database.label.toLowerCase()} does not answer, so the daemon cannot store the value.${
          dependencies.value.database.detail
            ? ` ${dependencies.value.database.detail}`
            : ''
        }`
      : 'The daemon does not answer, so the daemon cannot store the value.';

  const handleToggle = $(async (next: boolean) => {
    pending.value = true;
    queueError.value = null;
    const result = await props.onUpdateSettings$({ queueEnabled: next });
    pending.value = false;
    if (result.failed) {
      queueError.value = result.message;
      return;
    }
    settings.value = result.data;
  });

  const handleSaveResolver = $(async (input: ResolverSettingsInput) => {
    pending.value = true;
    resolverError.value = null;
    resolverSaved.value = false;
    const result = await props.onUpdateSettings$(input);
    if (result.failed) {
      pending.value = false;
      resolverError.value = result.message;
      return;
    }
    settings.value = result.data;
    resolverSaved.value = true;

    // The state of the resolver depends on the settings: the selected
    // model, the device, and the label budget all move with them. The
    // dependencies move too, because the address or the server may have
    // changed.
    const [fresh, places] = await Promise.all([
      props.onReadResolverStatus$(),
      props.onReadDependencies$(),
    ]);
    pending.value = false;
    if (!fresh.failed) {
      status.value = fresh.data;
    }
    if (!places.failed) {
      dependencies.value = places.data;
    }
  });

  /**
   * Store the sentences the user tries against the resolver.
   *
   * The list travels on its own, so adding a sentence never writes the
   * resolver form and never marks the form as saved. A failure of the
   * write leaves the list in front of the user, because the sentence they
   * just wrote is worth more than the stored list.
   */
  const handleStoreSentences = $(async (sentences: string[]) => {
    const result = await props.onUpdateSettings$({
      previewSentences: sentences,
    });
    if (!result.failed) {
      settings.value = { ...settings.value, previewSentences: sentences };
    }
  });

  const handleReadStatus = $(async () => {
    const result = await props.onReadResolverStatus$();
    if (!result.failed) {
      status.value = result.data;
    }
  });

  const handleDownloadModel = $(async (id: string) => {
    installing.value = true;
    resolverError.value = null;
    resolverSaved.value = false;
    const result = await props.onDownloadModel$(id);
    installing.value = false;
    if (result.failed) {
      resolverError.value = result.message;
      return;
    }
    status.value = result.data;
  });

  const handleRemoveModel = $(async (id: string) => {
    installing.value = true;
    resolverError.value = null;
    resolverSaved.value = false;
    const result = await props.onRemoveModel$(id);
    installing.value = false;
    if (result.failed) {
      resolverError.value = result.message;
      return;
    }
    status.value = result.data;
  });

  const handleDownloadLocalModel = $(async (id: string) => {
    installing.value = true;
    resolverError.value = null;
    resolverSaved.value = false;
    const result = await props.onDownloadLocalModel$(id);
    installing.value = false;
    if (result.failed) {
      resolverError.value = result.message;
      return;
    }
    status.value = result.data;
  });

  const handleRemoveLocalModel = $(async (id: string) => {
    installing.value = true;
    resolverError.value = null;
    resolverSaved.value = false;
    const result = await props.onRemoveLocalModel$(id);
    installing.value = false;
    if (result.failed) {
      resolverError.value = result.message;
      return;
    }
    status.value = result.data;
  });

  const handleEditResolver = $(() => {
    resolverSaved.value = false;
  });

  const handleEdit = $((id: string | null) => {
    editingId.value = id;
    formOpen.value = true;
    intentError.value = null;
  });

  const handleCloseForm = $(() => {
    formOpen.value = false;
    editingId.value = null;
    intentError.value = null;
  });

  const handleSaveIntent = $(async (input: IntentInput) => {
    pending.value = true;
    intentError.value = null;
    const id = editingId.value;
    const result = id
      ? await props.onUpdateIntent$({ id, intent: input })
      : await props.onCreateIntent$(input);
    pending.value = false;
    if (result.failed) {
      intentError.value = result.message;
      return;
    }
    intents.value = result.data.items;
    formOpen.value = false;
    editingId.value = null;
  });

  const handleAddExamples = $(async () => {
    pending.value = true;
    intentError.value = null;
    const result = await props.onAddExampleIntents$();
    pending.value = false;
    if (result.failed) {
      intentError.value = result.message;
      return;
    }
    intents.value = result.data.items;
  });

  const handleDeleteIntent = $(async (id: string) => {
    pending.value = true;
    intentError.value = null;
    const result = await props.onDeleteIntent$(id);
    pending.value = false;
    if (result.failed) {
      intentError.value = result.message;
      return;
    }
    intents.value = result.data.items;
    if (editingId.value === id) {
      formOpen.value = false;
      editingId.value = null;
    }
  });

  // The bar names every section and the state of the value it holds, so a
  // reader sees what a section is set to without opening it.
  const sections = [
    {
      id: 'resolver',
      label: 'Intent resolver',
      // The layered router is the engine now, so the bar names the state
      // that matters instead of a choice that no longer exists.
      status: { label: 'Router' },
    },
    {
      id: 'queue',
      label: 'Queue',
      status: settings.value.queueEnabled
        ? { label: 'On', tone: 'ok' as const }
        : { label: 'Off', tone: 'warn' as const },
    },
    {
      id: 'intents',
      label: 'Intents',
      status: { label: `${intents.value.length}` },
    },
  ];

  const handleSelectSection = $((id: string) => {
    section.value = readSettingsSection(id) as SettingsSection;
  });

  return (
    // The shell pins the viewport and never scrolls the document, so the
    // content owns its own scroll region. The bar sits outside it, so the
    // way between the sections stays in view while a long one moves.
    <Box class="flex min-h-0 flex-1 flex-col">
      {/* The name of the surface stands over the whole width, so the bar
          and the section below it share one heading. */}
      <Box class="w-full px-8 pt-6">
        <PageHeader
          title="Settings"
          description="Control the daemon queue, the intent resolver, and the intents."
        />
      </Box>

      <Box class="flex min-h-0 flex-1 flex-col gap-6 px-8 py-6 lg:flex-row">
        <SectionNav
          ariaLabel="Settings sections"
          items={sections}
          selected={section.value}
          onSelect$={handleSelectSection}
        />
        {/*
        The width leaves room for a section that places a diagram and the
        values of the picked step side by side on a wide screen.
      */}
        {/*
        The content is a column that fills the room under the head, so a
        section that draws a tall surface (the flow, the list of intents)
        takes the rest of the screen instead of a box of its own height.
        The column takes the whole width the bar leaves: a wide screen puts
        the room into the flow and into the values of the picked step rather
        than into the margin beside them.
      */}
        <ScrollArea ariaLabel="Settings" class="min-h-0 flex-1">
          <Stack gap="lg" class="h-full w-full">
            {/* The section names itself, so a reader always knows which
                part of the surface is in view. */}
            <Stack gap="xs" class="shrink-0">
              <Text size="title" weight="medium" block>
                {SECTION_TEXT[section.value].title}
              </Text>
              <Text size="body" tone="muted" block>
                {SECTION_TEXT[section.value].note}
              </Text>
            </Stack>

            {dependencies.value && !dependencies.value.database.reachable ? (
              <Alert
                tone="error"
                title="The database does not answer"
                message={`The daemon answered, but it cannot reach the database, so it cannot read or store a setting. The values on this page come from the daemon configuration.${
                  dependencies.value.database.detail
                    ? ` ${dependencies.value.database.detail}`
                    : ''
                }`}
              />
            ) : null}

            {section.value === 'resolver' ? (
              <ResolverSettingsSection
                settings={settings.value}
                status={status.value}
                dependencies={dependencies.value}
                pending={pending.value}
                installing={installing.value}
                error={resolverError.value}
                saved={resolverSaved.value}
                onSave$={handleSaveResolver}
                onDownloadModel$={handleDownloadModel}
                onRemoveModel$={handleRemoveModel}
                onDownloadLocalModel$={handleDownloadLocalModel}
                onRemoveLocalModel$={handleRemoveLocalModel}
                onRefreshStatus$={handleReadStatus}
                onEdit$={handleEditResolver}
                onPreviewMessage$={props.onPreviewMessage$}
                onStoreSentences$={handleStoreSentences}
              />
            ) : null}

            {section.value === 'queue' ? (
              <QueueToggleSection
                enabled={settings.value.queueEnabled}
                pending={pending.value}
                error={queueError.value}
                blocked={!canStore}
                blockedReason={storeReason}
                onToggle$={handleToggle}
              />
            ) : null}

            {section.value === 'intents' ? (
              // The list keeps the content and the form of one intent takes
              // the panel, so writing an intent never hides the list.
              <Box class="flex min-h-0 flex-1 flex-col gap-6 lg:flex-row">
                <IntentListSection
                  intents={intents.value}
                  pending={pending.value}
                  disabled={!canStore}
                  reason={storeReason}
                  editingId={editingId.value}
                  onEdit$={handleEdit}
                  onDelete$={handleDeleteIntent}
                  onAddExamples$={handleAddExamples}
                />
                <SidePanel
                  ariaLabel="The editor of one intent"
                  title={editing ? `Edit ${editing.name}` : 'New intent'}
                  note="The name, the entities, and the command of one intent."
                  open={formOpen.value}
                  onClose$={handleCloseForm}
                >
                  {formOpen.value ? (
                    // The key gives every intent its own form, so the drafts
                    // of one intent never leak into another.
                    <IntentFormSection
                      key={editingId.value ?? 'new'}
                      intent={editing}
                      frame={false}
                      pending={pending.value}
                      error={intentError.value}
                      onSave$={handleSaveIntent}
                      onPreviewScript$={props.onPreviewScript$}
                      onCancel$={handleCloseForm}
                    />
                  ) : null}
                </SidePanel>
              </Box>
            ) : null}
          </Stack>
        </ScrollArea>
      </Box>
    </Box>
  );
});
