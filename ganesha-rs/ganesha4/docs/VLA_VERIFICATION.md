# VLA Action Verification Design

## Overview
After executing any GUI action, Ganesha must verify the action succeeded before proceeding.

## Verification Checks (in order)

### 1. Element State Transition
- Query target element properties post-action
- Compare to expected state (checked, disabled, selected, etc.)
- Timeout: 500ms

### 2. Visual Diff Confirmation  
- Screenshot region around target before/after
- Perceptual hash comparison
- Threshold: >5% difference expected for successful action

### 3. Error Detection
- Scan screen for error dialogs/toasts
- Check for red highlights, warning icons
- Abort if unexpected error appears

### 4. Focus Verification
- Confirm focus moved as expected (for clicks/tabs)
- Input fields should have cursor
- Buttons should show pressed state

### 5. Content Verification
- For type actions: verify text appears in field
- For selections: verify dropdown shows selected value
- For file dialogs: verify path updated

## Retry Policy
- Max retries: 3
- Backoff: 200ms, 500ms, 1000ms
- Different strategy each retry (click center vs edge, etc.)

## Failure Handling
- Screenshot on failure
- Log full action context
- Offer rollback if checkpoint exists
- Ask user for guidance if ambiguous

## Implementation Status
- [ ] Element state queries (ganesha-vision)
- [ ] Visual diff (needs image comparison lib)
- [ ] Error detection prompts
- [ ] Focus tracking
- [ ] Content verification
- [ ] Retry logic
- [ ] Failure screenshots
