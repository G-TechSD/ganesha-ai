#!/usr/bin/env python3
"""
Test the Claudia Bridge with a sample HyperHealth packet.

This simulates what happens when Claudia queues a work packet
and Ganesha executes it using local LLMs.
"""

from pathlib import Path
from claudia_bridge import ClaudiaBridge, WorkPacket

# Sample HyperHealth packet (simulating Linear import)
sample_packet = WorkPacket(
    id="HYP-42",
    title="Medication Reminder System",
    description="""Create a medication reminder component that:
- Shows upcoming medication schedules
- Allows users to mark medications as taken
- Sends notifications at scheduled times
- Tracks medication adherence history""",
    phase="features",
    project_id="hyperhealth-beta",
    project_name="HyperHealth Beta",
    tasks=[
        {"description": "Create MedicationReminder component"},
        {"description": "Add medication schedule display"},
        {"description": "Implement 'Mark as Taken' functionality"},
        {"description": "Add adherence tracking"}
    ],
    acceptance_criteria=[
        "Component renders medication list",
        "Users can mark medications as taken",
        "Shows next scheduled reminder time",
        "Displays adherence percentage",
        "Uses shadcn/ui components"
    ],
    tech_stack=["Next.js", "TypeScript", "Tailwind CSS", "shadcn/ui"]
)


def main():
    print("="*60)
    print("CLAUDIA-GANESHA INTEGRATION TEST")
    print("Project: HyperHealth Beta")
    print("="*60)

    # Initialize bridge with local LLMs
    bridge = ClaudiaBridge()

    # Check available providers
    available = bridge.provider_chain.get_available_providers()
    print(f"\nAvailable LLM providers: {len(available)}")
    for p in available:
        url = getattr(p, 'url', 'cloud')
        print(f"  - {p.name}: {url}")

    # Execute the packet
    print(f"\n{'='*60}")
    print(f"Executing: {sample_packet.title}")
    print(f"{'='*60}")
    print(f"Description: {sample_packet.description[:100]}...")
    print(f"Tasks: {len(sample_packet.tasks)}")
    print(f"Criteria: {len(sample_packet.acceptance_criteria)}")

    result = bridge.execute_packet(
        sample_packet,
        max_iterations=2,  # Quick test
        min_confidence=0.6,
        debug=True
    )

    # Show results
    print(f"\n{'='*60}")
    print("EXECUTION RESULT")
    print("="*60)
    print(f"Success: {result.success}")
    print(f"Files generated: {len(result.files)}")
    print(f"Iterations: {result.iterations}")
    print(f"Confidence: {result.confidence*100:.0f}%")
    print(f"Duration: {result.duration_ms}ms")
    print(f"Model: {result.model_used}")
    print(f"Provider: {result.provider_used}")

    if result.files:
        print("\nGenerated files:")
        for f in result.files:
            print(f"  - {f['path']} ({len(f['content'])} bytes)")

        # Write to test output directory
        output_dir = Path("/tmp/hyperhealth-test")
        written = bridge.write_files_to_disk(result, output_dir)
        print(f"\nWritten to: {output_dir}")

        # Show first file content
        if result.files:
            first_file = result.files[0]
            print(f"\n--- {first_file['path']} ---")
            print(first_file['content'][:1500])
            if len(first_file['content']) > 1500:
                print("... (truncated)")

    if result.errors:
        print(f"\nErrors: {result.errors}")


if __name__ == "__main__":
    main()
