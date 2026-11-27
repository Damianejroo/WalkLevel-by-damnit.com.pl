<script lang="ts">
  // Stand-in for the level badge when an account hides its level: the first letter of the
  // nickname in a neutral 34 px circle (docs/design.md 8.16).
  interface Props {
    name: string;
    /** Screen reader name; without it the circle is decoration. */
    label?: string;
    title?: string;
  }
  let { name, label, title }: Props = $props();
  const initial = $derived(([...name.trim()][0] ?? '?').toLocaleUpperCase('pl-PL'));
</script>

<span
  class="initial-avatar"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
  {title}>{initial}</span>

<style>
  .initial-avatar {
    display: grid;
    place-items: center;
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: var(--r-round);
    background: var(--surface-2);
    color: var(--text);
    font-size: var(--fs-base);
    font-weight: var(--fw-bold);
    line-height: 1;
    user-select: none;
  }
</style>
