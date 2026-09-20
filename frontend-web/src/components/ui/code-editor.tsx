/**
 * `CodeEditor` is a CodeMirror 6 wrapper with HUD styling.
 *
 * It supports read-only display with JSON / shell highlighting and an
 * editable mode for the Try it out flow. The editor lives only in the
 * browser, so it mounts in a `useVisibleTask$`.
 */
import type { Extension } from '@codemirror/state';
import type { EditorView as EditorViewType } from '@codemirror/view';

import type { QRL } from '@builder.io/qwik';

import { component$, useSignal, useVisibleTask$ } from '@builder.io/qwik';

import { Box } from '~/components/ui/box';

export type CodeLanguage = 'json' | 'bash' | 'shell' | 'yaml';

/** The props of `CodeEditor`. */
export interface CodeEditorProps {
  /** The code to show. */
  code: string;
  /** The language for highlighting. */
  language?: CodeLanguage;
  /** Make the editor read-only. */
  readOnly?: boolean;
  /** The max height before scrolling. */
  maxHeight?: string;
  /** Called on every edit. */
  onChange$?: QRL<(value: string) => void>;
  /** Extra class for the outer Box. */
  class?: string;
}

export const CodeEditor = component$<CodeEditorProps>((props) => {
  const ref = useSignal<HTMLDivElement>();
  const viewRef = useSignal<EditorViewType | null>(null);

  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(async ({ track, cleanup }) => {
    track(() => props.readOnly);
    track(() => props.language);

    const container = ref.value;
    if (!container) return;

    // Clear previous
    container.textContent = '';

    const [
      { EditorView, keymap },
      { EditorState, Compartment },
      { json },
      { shell },
      { oneDark },
    ] = await Promise.all([
      import('@codemirror/view'),
      import('@codemirror/state'),
      import('@codemirror/lang-json'),
      import('@codemirror/legacy-modes/mode/shell').then((m) => ({
        shell: m.shell,
      })),
      import('@codemirror/theme-one-dark'),
    ]);
    const { StreamLanguage } = await import('@codemirror/language');

    const languageConf = new Compartment();
    const readOnlyConf = new Compartment();
    const themeConf = new Compartment();

    const languageExt: Extension =
      props.language === 'json'
        ? json()
        : props.language === 'bash' || props.language === 'shell'
          ? StreamLanguage.define(shell)
          : [];
    const lineWrapping: Extension = (
      EditorView as unknown as { lineWrapping: Extension }
    ).lineWrapping;

    const hudTheme = EditorView.theme({
      '&': { backgroundColor: 'transparent', fontSize: '11px' },
      '.cm-content': {
        fontFamily: 'var(--ds-font-hud)',
        color: 'var(--ds-color-text)',
      },
      '.cm-gutters': {
        backgroundColor: 'transparent',
        borderRight: '1px solid rgb(255 255 255 / 0.05)',
        color: 'var(--ds-color-text-faint)',
      },
      '.cm-cursor': { borderLeftColor: 'var(--ds-color-accent)' },
      '.cm-selectionBackground, ::selection': {
        backgroundColor: 'rgb(255 111 77 / 0.2)',
      },
    });

    const update = EditorView.updateListener.of((tr) => {
      if (tr.docChanged && props.onChange$) {
        const next = tr.state.doc.toString();
        void (props.onChange$ as QRL<(value: string) => void>)(next);
      }
    });

    const state = EditorState.create({
      doc: props.code,
      extensions: [
        lineWrapping,
        languageConf.of(languageExt),
        readOnlyConf.of(EditorState.readOnly.of(!!props.readOnly)),
        themeConf.of([hudTheme, oneDark]),
        update,
        keymap.of([]),
      ],
    });

    const view = new EditorView({ state, parent: container });
    viewRef.value = view;
    // Force HUD background transparent over oneDark
    container
      .querySelector('.cm-editor')
      ?.setAttribute('style', 'background: transparent');

    cleanup(() => {
      view.destroy();
      viewRef.value = null;
    });
  });

  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ track }) => {
    track(() => props.code);
    const view = viewRef.value;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== props.code) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: props.code },
      });
    }
  });

  return (
    <Box
      class={`overflow-hidden rounded-ds-sm border border-ds-line-ultra bg-ds-surface-sunken ${props.class ?? ''}`}
    >
      {props.language ? (
        <Box class="flex items-center justify-between border-b border-ds-line-ultra bg-ds-surface-sunken px-3 py-1.5">
          <span class="font-ds-hud text-[10px] uppercase tracking-[0.16em] text-ds-text-faint">
            {props.language}
          </span>
        </Box>
      ) : null}
      <div
        ref={ref}
        class="cm-hud min-h-12 overflow-auto p-1 font-ds-hud text-[11px] leading-[1.6]"
        style={props.maxHeight ? `max-height:${props.maxHeight};` : undefined}
        aria-label={props.language ? `${props.language} code` : 'code'}
      />
    </Box>
  );
});
