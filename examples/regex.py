def epsilon_closure(transition, cur):
    sts = cur
    for _ in range(10):
        next = sts
        for i in range(1, 30):
            st = sts[i]
            if st and transition.get(('*', i)) != None:
                next[transition['*', i]] = 1
        sts = next
    return sts

def regex(s, p):
    transition = {}
    state = 1
    for (i, c) in enumerate(p):
        if c == '*':
            transition['*', state] = state + 1
            state += 1
        elif i < len(p) - 1 and p[i+1] == '*':
            transition[c, state] = state
        else:
            transition[c, state] = state + 1
            state += 1

    accept = state
    init = [0, 1] + [0] * 28
    cur = epsilon_closure(transition, init)
    for c in s:
        next = [0] * 30
        for i in range(1, 20):
            st = cur[i]
            if st:
                if transition.get((c, i)) != None:
                    next[transition[c, i]] = 1
                elif transition.get(('.', i)) != None:
                    next[transition['.', i]] = 1
        cur = epsilon_closure(transition, next)

    return cur[accept]