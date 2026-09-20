/**
 * Join class names and drop the empty parts.
 *
 * A primitive uses it to merge its own utilities with the ones the caller
 * passes through the `class` prop.
 */
export function joinClassNames(
  ...parts: Array<string | undefined | null | false>
): string {
  return parts.filter(Boolean).join(' ');
}
