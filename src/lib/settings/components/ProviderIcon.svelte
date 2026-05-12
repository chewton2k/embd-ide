<script lang="ts">
  import Icon from '@iconify/svelte';
  import type { AiProvider } from '../../modules/stores';

  interface Props {
    provider: AiProvider;
    size?: number;
  }

  let { provider, size = 14 }: Props = $props();

  const ICON: Record<AiProvider, string> = {
    openrouter: 'simple-icons:openrouter',
    openai:     'simple-icons:openai',
    anthropic:  'simple-icons:anthropic',
  };

  const COLOR: Record<AiProvider, string> = {
    openrouter: '#cdd6f4',
    openai:     '#10a37f',
    anthropic:  '#d97757',
  };
</script>

<span class="wrap" style="width: {size}px; height: {size}px; color: {COLOR[provider]};">
  <Icon icon={ICON[provider]} width={size} height={size} />
</span>

<style>
  .wrap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
</style>
