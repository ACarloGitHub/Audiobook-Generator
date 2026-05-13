# audiobook_generator/setup_helpers/user_prompts.py

def yes_no_prompt(prompt):
    """Asks the user for a yes/no response."""
    while True:
        response = input(prompt + " (y/n): ").strip().lower()
        if response in ('y', 'yes'):
            return True
        elif response in ('n', 'no'):
            return False
        print("Please answer 'y' or 'n'.")

def choice_prompt(prompt, options):
    """Asks the user to choose from numbered options."""
    while True:
        print(prompt)
        for i, (key, desc) in enumerate(options, start=1):
            print(f"[{i}] {desc}")
        try:
            choice = int(input("Choice: ").strip())
            if 1 <= choice <= len(options):
                return options[choice - 1][0]
            else:
                print(f"Invalid choice. Enter a number between 1 and {len(options)}.")
        except ValueError:
            print("Please enter a valid number.")