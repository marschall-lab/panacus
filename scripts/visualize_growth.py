#!/usr/bin/env python3

#
# std import
#
from argparse import ArgumentParser, ArgumentDefaultsHelpFormatter as ADHF
from sys import stdout, stderr, exit
from os import fdopen, path

#
# third party packages
#

import pandas as pd
import matplotlib.pyplot as plt
import numpy as np

from sklearn.linear_model import LinearRegression
from scipy.optimize import curve_fit
import seaborn as sns



def plot(df, fname, out):

    # setup fancy plot look
    sns.set_theme(style='darkgrid')
    sns.set_color_codes('bright')
    sns.set_palette('Paired')
    sns.despine(left=True, bottom=True)

    # let's do it!
    plt.figure(figsize=(15, 8))
    xticks = list(map(str, df.index))
    
    import pdb; pdb.set_trace() 
    plt.bar(x=xticks, height=df.cumulative)
    plt.bar(x=xticks, height=df.consensus, hatch='\\\\', edgecolor='black')
    plt.bar(x=xticks, height=df.common, hatch='////', edgecolor='black')

    quad = lambda x, *y: y[0]*x**y[1]
    Y = np.array(df.loc[:, 'cumulative'])
    X = np.arange(Y.shape[0])+1
    Xp = np.arange(df.shape[0])+1
    popt, pcov = curve_fit(quad, X, Y, p0=[1, 1])
    x = plt.plot(Xp-1, quad(Xp, *popt), '--',  color='black')

    plt.title(f'Pangenome growth ({fname})')
    _ = plt.xticks(rotation=65)
    plt.ylabel('#bp')
    plt.xlabel('samples')
    plt.legend(x, [f'least-squares fit to m*X^γ (m={popt[0]:.3f}, γ={popt[1]:.3f})'])
    plt.savefig(out, format='pdf')
    plt.close()


if __name__ == '__main__':
    description='''
    Visualize growth stats. PDF file will be plotted to stdout.
    '''
    parser = ArgumentParser(formatter_class=ADHF, description=description)
    parser.add_argument('growth_stats', type=open,
            help='Growth table computed by panacus')

    args = parser.parse_args()
    df = pd.read_csv(args.growth_stats, sep='\t', header=[1,2], index_col=[0])
    with fdopen(stdout.fileno(), 'wb', closefd=False) as out:
        plot(df, path.basename(args.growth_stats.name), out)

