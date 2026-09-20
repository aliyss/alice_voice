/**
 * The view model of a download size.
 *
 * A model list shows the size of a download next to the name of the model,
 * so the user knows what one press on Download starts. Two sections show
 * such a list, so the format lives in one place.
 */

/**
 * Format a byte count for a person.
 *
 * A count below one thousand megabytes reads as megabytes, because a
 * download of a small model is a number a person compares with another.
 * A larger count reads as gigabytes.
 */
export function formatSize(bytes: number): string {
  const megabytes = bytes / (1024 * 1024);
  if (megabytes < 1000) {
    return `${Math.round(megabytes)} MB`;
  }
  return `${(megabytes / 1024).toFixed(1)} GB`;
}
