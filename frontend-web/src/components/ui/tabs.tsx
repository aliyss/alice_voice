/**
 * `Tabs` names the parts of one context that stand beside one another.
 *
 * The views of a tab strip belong to the same thing and do not replace one
 * another: the parts of a form, the groups of a catalog, the pages of one
 * record. Each view holds its own content, so the strip is a way through a
 * long surface rather than a choice of how one value is read.
 *
 * Use `ContentSwitcher` when the values are alternatives that read the
 * same content another way: the same stage read by another reader, the
 * same model on another device. A switch replaces what is shown; the tabs
 * name what else there is.
 *
 * The strip is as wide as the views it names, because the values of a
 * surface do not have to fill it. Pass `w-full` when the rule under the
 * strip should span the surface.
 *
 * The strip is the primitive; the view under it is the caller's, because
 * a tab panel holds whatever the surface holds. Wire the two together
 * with the exported ids, so a screen reader reaches the view from its
 * name:
 *
 * ```tsx
 * <Tabs items={items} selected={chosen.value} onSelect$={...} ariaLabel="..." />
 * <div
 *   role="tabpanel"
 *   id={tabPanelId(chosen.value)}
 *   aria-labelledby={tabId(chosen.value)}
 * >
 *   ...
 * </div>
 * ```
 *
 * Use `ContentSwitcher` instead when the pick is one value of a form
 * rather than another view of the surface.
 */
import type { QRL } from '@builder.io/qwik';

import { $, component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** One view the tabs offer. */
export interface TabItem {
  /** The id the surface names the view by. */
  id: string;
  /** The words the tab shows. */
  label: string;
  /** A count or a state beside the label, or null. */
  status?: string | null;
  /** Dim one view without hiding it. */
  disabled?: boolean;
}

/** The id of the tab of one view. */
export function tabId(id: string): string {
  return `tab-${id}`;
}

/** The id of the panel of one view. */
export function tabPanelId(id: string): string {
  return `tabpanel-${id}`;
}

/** The props of `Tabs`. */
export interface TabsProps {
  /** The label for a screen reader. It names the set of views. */
  ariaLabel: string;
  /** The views the strip offers, in the order it shows them. */
  items: TabItem[];
  /** The id of the view that is shown. */
  selected: string;
  /** Wrap the tabs onto more than one line instead of scrolling them. */
  wrap?: boolean;
  /** Report the view the user picked. */
  onSelect$: QRL<(id: string) => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Tabs = component$<TabsProps>((props) => {
  // The arrow keys walk the views of the strip and the home and end keys
  // jump to its ends, which is what a tab list answers to.
  const handleKey = $((event: KeyboardEvent) => {
    const enabled = props.items.filter((item) => !item.disabled);
    if (enabled.length === 0) {
      return;
    }
    const current = Math.max(
      0,
      enabled.findIndex((item) => item.id === props.selected),
    );
    let index: number | null = null;
    if (event.key === 'ArrowRight') {
      index = (current + 1) % enabled.length;
    } else if (event.key === 'ArrowLeft') {
      index = (current - 1 + enabled.length) % enabled.length;
    } else if (event.key === 'Home') {
      index = 0;
    } else if (event.key === 'End') {
      index = enabled.length - 1;
    }
    if (index === null) {
      return;
    }
    // The arrow keys would scroll the strip under it, so the key stops
    // where it is read. The handler is not asynchronous, so the call runs
    // in the same tick as the event.
    // eslint-disable-next-line qwik/no-async-prevent-default
    event.preventDefault();
    props.onSelect$(enabled[index].id);
  });

  return (
    <div
      role="tablist"
      aria-label={props.ariaLabel}
      class={joinClassNames(
        'flex w-fit max-w-full gap-1 border-b border-ds-line',
        props.wrap ? 'flex-wrap' : 'overflow-x-auto',
        props.class,
      )}
    >
      {props.items.map((item) => {
        const chosen = item.id === props.selected;

        return (
          <button
            key={item.id}
            type="button"
            role="tab"
            id={tabId(item.id)}
            aria-selected={chosen}
            aria-controls={tabPanelId(item.id)}
            tabIndex={chosen ? 0 : -1}
            disabled={item.disabled}
            class={joinClassNames(
              '-mb-px inline-flex items-center gap-2 border-b-2 px-3 py-2 font-ds-hud text-[10px] tracking-[0.16em] uppercase transition-colors duration-150 ease-out disabled:pointer-events-none disabled:opacity-40',
              chosen
                ? 'border-ds-accent-strong text-ds-text'
                : 'border-transparent text-ds-text-faint hover:text-ds-text-muted',
            )}
            onClick$={$(() => props.onSelect$(item.id))}
            onKeyDown$={handleKey}
          >
            {item.label}
            {item.status ? (
              <span class="text-ds-text-faint">{item.status}</span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
});
