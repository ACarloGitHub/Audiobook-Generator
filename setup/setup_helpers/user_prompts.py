# audiobook_generator/setup_helpers/user_prompts.py

def yes_no_prompt(prompt):
    """Chiede all'utente una risposta sì/no."""
    while True:
        response = input(prompt + " (y/n): ").strip().lower()
        if response in ('y', 'yes'):
            return True
        elif response in ('n', 'no'):
            return False
        print("Per favore rispondi 'y' o 'n'.")

def choice_prompt(prompt, options):
    """Chiede all'utente di scegliere tra opzioni numerate."""
    while True:
        print(prompt)
        for i, (key, desc) in enumerate(options, start=1):
            print(f"[{i}] {desc}")
        try:
            choice = int(input("Scelta: ").strip())
            if 1 <= choice <= len(options):
                return options[choice - 1][0]
            else:
                print(f"Scelta non valida. Inserisci un numero tra 1 e {len(options)}.")
        except ValueError:
            print("Inserisci un numero valido.")