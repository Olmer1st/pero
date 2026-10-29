<script lang="ts">
  import Books from "./components/Books.svelte";
  import SignIn from "./components/SignIn.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import { session } from "./lib/session.svelte.ts";
</script>

<header>
  <strong class="brand">Pero</strong>
  <StatusBar />
  <span class="spacer"></span>
  {#if session.device}
    <span class="muted">{session.device.name}</span>
    <button type="button" class="secondary" onclick={() => session.signOut()}>Sign out</button>
  {/if}
</header>

<main>
  {#if session.device}
    <Books />
  {:else}
    <SignIn />
  {/if}
</main>
