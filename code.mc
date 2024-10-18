PC = 0x00 ; program counter
D1 = 0x02
D2 = 0x04
D3 = 0x06
TMP = 0x08 ; temporary register
COPY_TMP = 0x0A ; copy pointer
SP = 0x0C ; stack pointer
LR = 0x0E

SET SP 0x10
SET COPY_TMP TMP

NOT A =
	MOV A D1
	SET A 0xFFFF
	SUB A D1

MOV A B =
	STR A COPY_TMP ; *COPY_TMP = A
	LOD B COPY_TMP ; B = *COPY_TMP

ADD A B =
	MOV B D1 
	SET D2 0xFFFF ; NOT D1
	SUB D2 D1
	SUB A  D2 ; a - (0xffff - b) a + b - 0xffff
	SET D1 0x01
	SUB A D1

SET D3 12
#print_mem D3
ADD D3 D3
#print_mem D3


NEQ A B =
	SUB A B

; COND in {0, 1}
JNE COND TO = 	  ; if !cond { jmp TO }
	MOV COND D1   ; if cond == 1 => D1 = 1
	ADD D1 D1     ; if cond == 1 => d1 == 2
	ADD D1 D1     ; if cond == 1 => d1 == 4
	ADD D1 COND   ; if cond == 1 => d1 == 5
	ADD D1 D1     ; if cond == 1 => d1 == 10
	ADD D1 D1     ; if cond == 1 => PC += 5
	MOV TO PC     ; jmp TO


PUSH X =
	SET D1 2
	ADD D1 SP ; D1 = 2 + SP
	STR X  SP ; mem[SP] = X
	MOV D1 SP

POP X =
	SET D1 2
	SUB SP D1
	LOD X  SP

SET D3 102
#print_mem PC D1 D2 D3 TMP COPY_TMP SP LR
PUSH D3
POP D3
