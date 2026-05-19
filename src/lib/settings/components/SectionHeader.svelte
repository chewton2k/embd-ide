<script lang="ts">
  interface Props {
    title: string;
    description?: string;
  }
  let { title, description = '' }: Props = $props();
</script>

<div class="section-header">
  <h1>{title}</h1>
  {#if description}<p>{description}</p>{/if}
</div>

<style>
  .section-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 4px;
  }
  h1 {
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.3px;
    color: var(--text-primary);
    margin: 0;
  }
  p {
    font-size: 12px;
    color: var(--text-muted);
    margin: 0;
    line-height: 1.5;
  }
</style>
