import subprocess,re,sys,json
sys.path.insert(0,sys.argv[1]); import pe
def run(init):
    out=subprocess.run(['objdump','-d','-M','intel','--start-address=%#x'%init,'--stop-address=%#x'%(init+0x1400),pe.EXE],capture_output=True,text=True).stdout
    stores={}; bstores={}; calls=[]; seen=False; regs={}; unk=[]; this={'ecx'}
    for line in out.splitlines()[7:]:
        ins=line.split('\t')[-1].strip()
        if ins.startswith('ret'): seen=True
        if 'int3' in ins and seen: break
        if re.match(r'mov\s+(e\w\w),ecx$',ins) and 'ecx' in this: this.add(ins.split()[1].split(',')[0])
        m=re.match(r'mov\s+DWORD PTR \[(\w+)\+0x([0-9a-f]+)\],(\w+)$',ins)
        if m and m.group(1) in this:
            m=re.match(r'mov\s+DWORD PTR \[\w+\+0x([0-9a-f]+)\],(\w+)$',ins)
            v=m.group(2); off=int(m.group(1),16)
            if v.startswith('0x'): stores[off]=int(v,16)
            elif v in regs: stores[off]=regs[v]
            else: unk.append(ins)
            continue
        m=re.match(r'mov\s+BYTE PTR \[(\w+)\+0x([0-9a-f]+)\],(\w+)$',ins)
        if m and m.group(1) in this:
            off=int(m.group(2),16); v=m.group(3)
            if v.startswith('0x'): bstores[off]=int(v,16)
            elif v in ('al','bl','cl','dl') and 'e'+v[0]+'x' in regs: bstores[off]=regs['e'+v[0]+'x']&0xff
            else: unk.append(ins)
            continue
        m=re.match(r'mov\s+(e[a-d]x|e[sd]i|ebp),0x([0-9a-f]+)$',ins)
        if m: regs[m.group(1)]=int(m.group(2),16); continue
        m=re.match(r'xor\s+(\w+),(\w+)$',ins)
        if m and m.group(1)==m.group(2): regs[m.group(1)]=0; continue
        m=re.match(r'(inc|dec)\s+(\w+)$',ins)
        if m and m.group(2) in regs: regs[m.group(2)]+=1 if m.group(1)=='inc' else -1; continue
        m=re.match(r'(add|sub)\s+(\w+),0x([0-9a-f]+)$',ins)
        if m and m.group(2) in regs: regs[m.group(2)]+=int(m.group(3),16)*(1 if m.group(1)=='add' else -1); continue
        m=re.search(r'call\s+0x([0-9a-f]+)',ins)
        if m: calls.append(m.group(1)); regs.clear(); this.discard('ecx'); continue
        m=re.match(r'\w+\s+(e[a-d]x|e[sd]i|ebp)\b',ins)
        if m and not ins.startswith(('test','cmp','push')): regs.pop(m.group(1),None)
    return stores,bstores,calls,unk
res={}
for vt,name,create,gcn,init in pe.vtables():
    if init in (0x446e30,0x4a28e0,0x408960): continue
    stores,bstores,calls,unk=run(init)
    res[name]=dict(vtable=hex(vt),init=hex(init),stores={hex(k):v for k,v in sorted(stores.items())},bytes={hex(k):v for k,v in sorted(bstores.items())},unknown=unk)
json.dump(res,open(sys.argv[2],'w'),indent=1)
for n,r in res.items():
    s={int(k,16):v for k,v in r['stores'].items()}
    anim=[k for k in s if 0x24c<=k<0x44c]
    print(n,r['init'],'animstores',len(anim), 'range',hex(min(anim)) if anim else '',hex(max(anim)) if anim else '', {hex(k):v for k,v in s.items() if not 0x24c<=k<0x44c}, r['unknown'][:2])
