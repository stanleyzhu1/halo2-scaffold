def get_instruction(pc, code_sequence):
    for x in code_sequence:
        if x[3] == pc:
            return x
    return [0,0,0,0]

def compile(p):
    ret = []
    num = 1
    for i in range(len(p)):
        if p[i] == '*':
            ret.append([0,0,0,0])
            ret.append([0,0,0,0])
            ret.append([0,0,0,0])
        elif i < len(p) - 1 and p[i+1] == '*':
            ret.append([2, num+1, num+3, num])
            num += 1
            ret.append([1, p[i], 0, num])
            num += 1
            ret.append([3, num-2, 0, num])
            num += 1
        else:
            ret.append([1, p[i], 0, num])
            num += 1
            ret.append([0,0,0,0])
            ret.append([0,0,0,0])
    ret.append([4,0,0,num])
    return ret

def regex_vm(s, p):
    code_sequence = compile(p)
    print(code_sequence)
    thread_list = [[0, 0]] * 16
    thread_list[0] = [1,0]
    avail = 1
    for j in range(16):
        print(thread_list)
        pc = thread_list[j][0]
        sp = thread_list[j][1]
        for i in range(len(s) * 4):
            inst = get_instruction(pc, code_sequence)
            print(pc)
            print(sp)
            # if pc == 7:
            #     print("Hi")
            #     print(inst)
            #     print(sp)
            if inst[0] == 0:
                break
            if inst[0] == 1:
                # print("Hello")
                if sp >= len(s) or (s[sp] != inst[1] and inst[1] != '.'):
                    pc = 0
                else:
                    pc += 1
                    sp += 1
            elif inst[0] == 2:
                pc = inst[1]
                thread_list[avail] = [inst[2], sp]
                avail += 1
            elif inst[0] == 3:
                pc = inst[1]
            elif inst[0] == 4:
                if sp == len(s):
                    return True

    return False