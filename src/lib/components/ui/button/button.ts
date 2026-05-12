import { cva, type VariantProps } from 'class-variance-authority';

export const buttonVariants = cva(
  'ui-button',
  {
    variants: {
      variant: {
        default: 'ui-button--default',
        ghost: 'ui-button--ghost',
        subtle: 'ui-button--subtle',
      },
      size: {
        sm: 'ui-button--sm',
        icon: 'ui-button--icon',
        md: 'ui-button--md',
      },
    },
    defaultVariants: {
      variant: 'ghost',
      size: 'sm',
    },
  }
);

export type ButtonVariant = VariantProps<typeof buttonVariants>;
