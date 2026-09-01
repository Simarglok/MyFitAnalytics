<script lang="ts">
  import AppShell from './lib/components/AppShell.svelte';
  import type { AppTransport } from './lib/transport';

  export let transport: AppTransport;
</script>

<AppShell {transport} />
