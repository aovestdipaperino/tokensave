<script module>
  export const prerender = true;
  export const ssr = false;
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    count?: number;
    children?: Snippet;
  }

  let { title, count = 0 }: Props = $props();
  let doubled = $derived(count * 2);

  export function increment(): void {
    count++;
  }

  export async function fetchData(url: string): Promise<string> {
    const res = await fetch(url);
    return res.text();
  }
</script>

<h1>{title}</h1>
<p>Count: {count}, doubled: {doubled}</p>
<button onclick={increment}>+1</button>
