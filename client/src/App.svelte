<script lang="ts">
  import BookPage from "./components/BookPage.svelte";
  import Books from "./components/Books.svelte";
  import SceneEditor from "./components/SceneEditor.svelte";
  import SignIn from "./components/SignIn.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import { router } from "./lib/route.svelte.ts";
  import { session } from "./lib/session.svelte.ts";

  function signOut(): void {
    if (router.guard !== null && !router.guard()) return;
    session.signOut();
  }
</script>

<header>
  <strong class="brand">Pero</strong>
  <StatusBar />
  <span class="spacer"></span>
  {#if session.device}
    <span class="muted">{session.device.name}</span>
    <button type="button" class="secondary" onclick={signOut}>Sign out</button>
  {/if}
</header>

<main class:wide={session.device && router.route.name === "scene"}>
  {#if session.device}
    {#if router.route.name === "book"}
      <BookPage id={router.route.id} tab={router.route.tab} />
    {:else if router.route.name === "scene"}
      <SceneEditor id={router.route.id} passage={router.route.passage} />
    {:else}
      <Books />
    {/if}
  {:else}
    <SignIn />
  {/if}
</main>
